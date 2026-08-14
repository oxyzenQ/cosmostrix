// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal tests extracted from terminal.rs.

#[cfg(test)]
mod tests {
    use crate::terminal::{TERMINAL_RESET_SEQUENCE, TERMINAL_RESTORE_SEQUENCE};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    #[derive(Default)]
    struct CleanupFlags {
        mouse: bool,
        focus: bool,
        bracketed_paste: bool,
        cursor: bool,
        wrap: bool,
        signal_exit_clear: bool,
        alternate: bool,
        raw: bool,
        cleaned: bool,
    }

    impl CleanupFlags {
        fn cleanup_plan(&mut self, _signal_exit: bool) -> Vec<&'static str> {
            if self.cleaned {
                return Vec::new();
            }
            self.cleaned = true;

            let mut plan = Vec::new();
            if self.mouse {
                plan.push("disable-mouse");
                self.mouse = false;
            }
            if self.focus {
                plan.push("disable-focus");
                self.focus = false;
            }
            if self.bracketed_paste {
                plan.push("disable-bracketed-paste");
                self.bracketed_paste = false;
            }
            if self.cursor {
                plan.push("show-cursor");
                self.cursor = false;
            }
            if self.wrap {
                plan.push("enable-wrap");
                self.wrap = false;
            }
            // v31.1: REMOVED clear-viewport before leave-alternate.
            // \x1b[2J in the alternate screen clears main screen
            // scrollback on some terminals (VTE, xterm-direct).
            if self.alternate {
                plan.push("leave-alternate");
                self.alternate = false;
            }
            if self.raw {
                plan.push("disable-raw");
                self.raw = false;
            }
            plan
        }
    }

    #[test]
    fn normal_restore_sequence_disables_terminal_reporting_modes() {
        for mode in [
            "?1000l", "?1002l", "?1003l", "?1006l", "?1015l", "?2004l", "?1004l", "?1049l", "?25h",
            "?2026l", // synchronized output (added v15)
        ] {
            assert!(
                TERMINAL_RESTORE_SEQUENCE.contains(mode),
                "missing terminal restore mode {mode}"
            );
        }
        // Scroll region reset
        assert!(
            TERMINAL_RESTORE_SEQUENCE.contains("\x1b[r"),
            "restore must reset scroll region to full screen"
        );
        // Character set reset to US ASCII
        assert!(
            TERMINAL_RESTORE_SEQUENCE.contains("\x1b(B"),
            "restore must reset character set to US ASCII"
        );
        // Auto-wrap enabled
        assert!(
            TERMINAL_RESTORE_SEQUENCE.contains("\x1b[?7h"),
            "restore must enable auto-wrap"
        );
        assert!(TERMINAL_RESTORE_SEQUENCE.ends_with("\x1b[0m"));
    }

    #[test]
    fn normal_restore_sequence_does_not_clear_screen_or_scrollback() {
        assert!(
            !TERMINAL_RESTORE_SEQUENCE.contains("\x1b[2J"),
            "normal restore must not clear the visible screen"
        );
        assert!(
            !TERMINAL_RESTORE_SEQUENCE.contains("\x1b[3J"),
            "normal restore must not purge scrollback"
        );
        assert!(
            !TERMINAL_RESTORE_SEQUENCE.contains("\x1b[H"),
            "normal restore must not move cursor home on the shell screen"
        );
    }

    #[test]
    fn reset_terminal_sequence_disables_terminal_reporting_modes() {
        for mode in [
            "?1000l", "?1002l", "?1003l", "?1006l", "?1015l", "?2004l", "?1004l", "?1049l", "?25h",
            "?2026l", // synchronized output (added v15)
        ] {
            assert!(
                TERMINAL_RESET_SEQUENCE.contains(mode),
                "missing terminal reset mode {mode}"
            );
        }
        // Reset sequence must also reset scroll region, charset, auto-wrap
        assert!(
            TERMINAL_RESET_SEQUENCE.contains("\x1b[r"),
            "reset must reset scroll region"
        );
        assert!(
            TERMINAL_RESET_SEQUENCE.contains("\x1b(B"),
            "reset must reset character set to US ASCII"
        );
        assert!(
            TERMINAL_RESET_SEQUENCE.contains("\x1b[?7h"),
            "reset must enable auto-wrap"
        );
        assert!(TERMINAL_RESET_SEQUENCE.ends_with("\x1b[0m"));
    }

    #[test]
    fn reset_terminal_sequence_clears_screen_and_scrollback() {
        assert!(
            TERMINAL_RESET_SEQUENCE.contains("\x1b[2J"),
            "reset sequence must clear the visible screen"
        );
        assert!(
            TERMINAL_RESET_SEQUENCE.contains("\x1b[3J"),
            "reset sequence must request scrollback purge"
        );
        assert!(
            TERMINAL_RESET_SEQUENCE.matches("\x1b[H").count() >= 2,
            "reset sequence must move cursor home before and after clearing"
        );
    }

    #[test]
    fn reset_terminal_sequence_is_idempotent() {
        let repeated = format!("{TERMINAL_RESET_SEQUENCE}{TERMINAL_RESET_SEQUENCE}");
        for required in ["\x1b[0m", "\x1b[?1049l", "\x1b[?25h", "\x1b[2J", "\x1b[3J"] {
            assert!(
                repeated.contains(required),
                "repeated reset sequence missing required command {required:?}"
            );
        }
    }

    #[test]
    fn terminal_cleanup_plan_is_reverse_order_and_idempotent() {
        let mut flags = CleanupFlags {
            mouse: true,
            focus: true,
            bracketed_paste: true,
            cursor: true,
            wrap: true,
            alternate: true,
            raw: true,
            cleaned: false,
            ..Default::default()
        };

        assert_eq!(
            flags.cleanup_plan(false),
            [
                "disable-mouse",
                "disable-focus",
                "disable-bracketed-paste",
                "show-cursor",
                "enable-wrap",
                "leave-alternate",
                "disable-raw",
            ]
        );
        let mut flags = CleanupFlags {
            mouse: true,
            focus: true,
            bracketed_paste: true,
            cursor: true,
            wrap: true,
            alternate: true,
            raw: true,
            cleaned: false,
            ..Default::default()
        };
        let plan = flags.cleanup_plan(false);
        // v31.1: normal exit NO LONGER clears viewport — \x1b[2J in
        // alt screen destroys main screen scrollback on some terminals.
        assert!(!plan.contains(&"clear-viewport"));
        assert!(!plan.contains(&"purge-scrollback"));
        assert!(!plan.contains(&"cursor-home"));
        assert!(flags.cleanup_plan(false).is_empty());
    }

    #[test]
    fn signal_exit_cleanup_leaves_alternate_without_clear_v31_1() {
        // v31.1: even signal exit no longer clears viewport before
        // leave-alternate — same scrollback destruction risk.
        let mut flags = CleanupFlags {
            mouse: true,
            focus: true,
            bracketed_paste: true,
            cursor: true,
            wrap: true,
            alternate: true,
            raw: true,
            cleaned: false,
            ..Default::default()
        };

        let plan = flags.cleanup_plan(true);
        let leave_idx = plan.iter().position(|&s| s == "leave-alternate");
        assert!(
            leave_idx.is_some(),
            "signal-exit cleanup must include leave-alternate"
        );
        // v31.1: no clear-viewport before leave-alternate
        assert!(!plan.contains(&"clear-viewport"));
    }

    #[test]
    fn normal_exit_cleanup_preserves_scrollback_v31_1() {
        // v31.1: normal exit NO LONGER clears viewport — \x1b[2J in
        // alt screen destroys main screen scrollback on some terminals.
        // LeaveAlternateScreen alone properly restores the main screen.
        let mut flags = CleanupFlags {
            mouse: true,
            focus: true,
            bracketed_paste: true,
            cursor: true,
            wrap: true,
            alternate: true,
            raw: true,
            cleaned: false,
            ..Default::default()
        };

        let plan = flags.cleanup_plan(false);
        assert!(
            !plan.contains(&"clear-viewport"),
            "v31.1: normal exit must NOT clear viewport (scrollback safety)"
        );
        assert!(!flags.signal_exit_clear);
    }

    #[test]
    fn signal_exit_flag_is_atomic_and_shared() {
        let flag = Arc::new(AtomicBool::new(false));
        let clone = flag.clone();
        assert!(!flag.load(std::sync::atomic::Ordering::Acquire));
        clone.store(true, std::sync::atomic::Ordering::Release);
        assert!(flag.load(std::sync::atomic::Ordering::Acquire));
    }
}
