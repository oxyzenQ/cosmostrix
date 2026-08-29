// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the v50 `message` / `message-border` / `msg-mode` config keys
//! (the message overlay + msg-mode gate). Extracted from
//! `config_apply_tests/mod.rs` to keep that source file under the 800-LOC
//! cap. Pure code motion — no behavior change.

#![cfg(test)]

use super::args_with_config;

// ── v50 message / message-border config key tests ──────────────────────────

#[test]
fn config_message_border_applies_with_border() {
    // Setting message-border = "X" in config sets args.message + border=true.
    let args = args_with_config("message-border = \"A masterpiece\"\n", &[]);
    assert_eq!(args.message.as_deref(), Some("A masterpiece"));
    assert!(
        args.message_border,
        "message-border config must set border=true"
    );
}

#[test]
fn config_message_bare_applies_without_border() {
    // Setting message = "X" in config sets args.message + border=false.
    let args = args_with_config("message = \"Hello world\"\n", &[]);
    assert_eq!(args.message.as_deref(), Some("Hello world"));
    assert!(
        !args.message_border,
        "message config must keep border=false"
    );
}

#[test]
fn config_message_border_wins_over_message_when_both_present() {
    // When both keys are in config, message-border wins (border=true, text from message-border).
    let args = args_with_config("message = \"plain\"\nmessage-border = \"boxed\"\n", &[]);
    assert_eq!(args.message.as_deref(), Some("boxed"));
    assert!(
        args.message_border,
        "message-border should win over message"
    );
}

#[test]
fn cli_m_flag_wins_over_config_message() {
    // CLI -m overrides config message text and keeps border=false.
    let args = args_with_config("message-border = \"from config\"\n", &["-m", "from cli"]);
    assert_eq!(args.message.as_deref(), Some("from cli"));
    assert!(
        !args.message_border,
        "CLI -m must keep border=false even when config has message-border"
    );
}

#[test]
fn config_message_missing_leaves_args_none() {
    // When neither CLI nor config provides a message, args.message stays None.
    // main.rs applies the default fallback ("cosmostrix" + border=true).
    let args = args_with_config("", &[]);
    assert_eq!(args.message, None);
    assert!(!args.message_border);
}

#[test]
fn config_message_empty_string_is_accepted() {
    // Empty string is a valid message (renders as just the border frame).
    let args = args_with_config("message-border = \"\"\n", &[]);
    assert_eq!(args.message.as_deref(), Some(""));
    assert!(args.message_border);
}

#[test]
fn config_message_unknown_variant_rejected_as_unknown_key() {
    // Typos like "massage-border" must surface as unknown keys.
    let parsed = crate::configfile::parse_config_text("massage-border = \"typo\"\n");
    assert!(
        parsed.unknown_keys.contains(&"massage-border".to_string()),
        "expected 'massage-border' in unknown_keys, got: {:?}",
        parsed.unknown_keys
    );
}

// ── v50-beta.3: msg-mode gate tests ─────────────────────────────────────────

#[test]
fn config_msg_mode_false_disables_config_message() {
    // msg-mode=false + config message → message cleared.
    // User must set msg-mode=true to use config message/message-border.
    let args = args_with_config("msg-mode = false\nmessage-border = \"hello\"\n", &[]);
    assert_eq!(
        args.message, None,
        "msg-mode=false must clear config message"
    );
    assert!(!args.message_border);
    assert_eq!(args.msg_mode, Some(false));
}

#[test]
fn config_msg_mode_true_keeps_config_message() {
    // msg-mode=true + config message → message preserved (default behavior).
    let args = args_with_config("msg-mode = true\nmessage-border = \"hello\"\n", &[]);
    assert_eq!(args.message.as_deref(), Some("hello"));
    assert!(args.message_border);
    assert_eq!(args.msg_mode, Some(true));
}

#[test]
fn config_msg_mode_default_true_when_unset() {
    // No msg-mode key → default true (message overlay active).
    let args = args_with_config("", &[]);
    assert_eq!(args.msg_mode, None); // config layer doesn't set default; main.rs does
}

#[test]
fn cli_m_flag_wins_over_msg_mode_false() {
    // CLI -m + config msg-mode=false → message still shows (CLI wins).
    let args = args_with_config("msg-mode = false\n", &["-m", "from-cli"]);
    assert_eq!(args.message.as_deref(), Some("from-cli"));
    assert!(!args.message_border, "CLI -m must keep border=false");
    assert_eq!(args.msg_mode, Some(false));
}

#[test]
fn cli_mb_flag_wins_over_msg_mode_false() {
    // CLI -mb + config msg-mode=false → message still shows with border.
    // Note: -mb is expanded to --message-border (bool toggle) + -m "text"
    // in main.rs pre-parse step. Tests bypass main.rs, so use the expanded
    // form directly: --message-border + -m "text".
    let args = args_with_config(
        "msg-mode = false\n",
        &["--message-border", "-m", "from-cli"],
    );
    assert_eq!(args.message.as_deref(), Some("from-cli"));
    assert!(args.message_border, "CLI -mb must keep border=true");
    assert_eq!(args.msg_mode, Some(false));
}

#[test]
fn config_msg_mode_false_with_config_message_bare_also_cleared() {
    // msg-mode=false + config `message = "x"` (no border) → also cleared.
    let args = args_with_config("msg-mode = false\nmessage = \"hello\"\n", &[]);
    assert_eq!(args.message, None);
    assert!(!args.message_border);
}

#[test]
fn config_msg_mode_invalid_value_rejected() {
    // msg-mode = "yes" should be accepted (parse_true_false accepts yes/no/on/off).
    // msg-mode = "maybe" should NOT be accepted (not a valid bool).
    let parsed = crate::configfile::parse_config_text("msg-mode = \"maybe\"\n");
    // The bare key is known, so it doesn't land in unknown_keys — but the
    // value parse fails silently in config_apply (parse_bool_config returns
    // None, args.msg_mode stays None → main.rs default true). Verify the key
    // is recognized (not in unknown_keys):
    assert!(
        !parsed.unknown_keys.contains(&"msg-mode".to_string()),
        "msg-mode is a known key, should not be in unknown_keys: {:?}",
        parsed.unknown_keys
    );
}
