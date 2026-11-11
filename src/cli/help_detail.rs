// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! CLI --help print path: the single help surface (v30 simplify — the
//! former `--help-detail` advanced reference and the top-level help
//! merged into one; `--help` is the only reference surface).
//!
//! night-T-1 (2026-11-11): the HELP_TEXT constant moved to
//! `help_text.rs` — this file sat exactly at the 800-line hard cap
//! (src/RULES_LOC.md) and the zelynic-shaped help head needed room.
//! The file name is kept as `help_detail.rs` for git-blame
//! continuity; the public function is still `print_help()`, and the
//! NIGHT-boost-2 hygiene tests below still assert on the constant's
//! layout from here.

use super::help_text::HELP_TEXT;

pub(crate) fn print_help() {
    if crate::config::color_enabled_stdout() {
        print!("{}", crate::config::colorize_help(HELP_TEXT));
    } else {
        print!("{}", HELP_TEXT);
    }
}

// v50 (honesty audit): verify help text drift parameters match the
// source-of-truth constants. If someone retunes the constants without
// updating the help text, this test catches the drift.
#[cfg(test)]
mod honesty_tests {
    use crate::central_control_rains::COLOR_ECOSYSTEM_TICK_SECS;

    #[test]
    fn help_text_drift_params_match_constants() {
        // The help text mentions the ecosystem tick rate.
        // Verify it matches the actual constant.
        assert_eq!(
            COLOR_ECOSYSTEM_TICK_SECS, 3.0,
            "help text says 3s tick but COLOR_ECOSYSTEM_TICK_SECS changed"
        );
    }
}

// NIGHT-boost-2 (2026-09-24) layout contract: `#` annotations live
// ABOVE their example CLI line, never as right-side comments. The old
// right-side form misaligned at every terminal width (comments drifted
// away from their command as soon as an example was longer than the
// author's screen) and read as part of the command itself.
#[cfg(test)]
mod example_layout_tests {
    /// No example CLI line may carry a right-side `#` comment.
    #[test]
    fn help_text_has_no_right_side_hash_comments() {
        for line in super::HELP_TEXT.lines() {
            if line.trim_start().starts_with("cosmostrix") {
                assert!(
                    !line.contains('#'),
                    "right-side `#` comment on example line (annotations belong ABOVE): {line}"
                );
            }
        }
    }

    /// NIGHT-boost-3: the -v section must disclose the session
    /// telemetry implication (one flag = config + behavior + telemetry)
    /// and the --perf-stats section must point out it is implied by
    /// --verbose — the two sections document the same contract from
    /// both ends.
    #[test]
    fn verbose_section_discloses_perf_report_implication() {
        assert!(
            super::HELP_TEXT.contains("NIGHT-boost-3: in interactive mode -v also implies"),
            "the -v section must disclose the exit PERFORMANCE REPORT implication"
        );
        assert!(
            super::HELP_TEXT.contains("implied by --verbose in interactive mode"),
            "the --perf-stats section must disclose the -v implication"
        );
    }

    /// The moved annotations must sit directly above their example
    /// line — content preserved, position changed. Spot-checks one
    /// 6-space Common options pair and one 8-space --dump-config pair.
    #[test]
    fn annotations_sit_directly_above_their_examples() {
        assert!(
            super::HELP_TEXT
                .contains("      # custom palette from config\n      cosmostrix -c cyberpunk_2077")
        );
        assert!(super::HELP_TEXT.contains(
            "        # blocked (shell redirect)\n        cosmostrix --dump-config > /tmp/a.txt"
        ));
    }
}
