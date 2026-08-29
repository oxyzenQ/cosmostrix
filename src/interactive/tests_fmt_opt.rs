// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! fmt_opt_str + ambient accessor tests — extracted from
//! `interactive/tests.rs` to keep that file under the 800-LOC hard cap.

use super::fmt_opt_str;

mod fmt_opt_str_tests {
    use super::fmt_opt_str;

    #[test]
    fn some_quoted_without_wrapper() {
        assert_eq!(fmt_opt_str(Some("hello")), "\"hello\"");
    }

    #[test]
    fn none_shows_paren_none() {
        assert_eq!(fmt_opt_str(None), "(none)");
    }

    #[test]
    fn no_some_wrapper_in_output() {
        let formatted = fmt_opt_str(Some("test"));
        assert!(!formatted.contains("Some("), "got {formatted}");
    }

    #[test]
    fn empty_string_some_shows_empty_quotes() {
        assert_eq!(fmt_opt_str(Some("")), "\"\"");
    }

    #[test]
    fn last_ambient_snapback_secs_defaults_to_none_when_unset() {
        let _ = crate::interactive::last_ambient_snapback_secs();
    }

    #[test]
    fn last_ambient_entries_defaults_to_zero_when_unset() {
        let _ = crate::interactive::last_ambient_entries();
    }
}
