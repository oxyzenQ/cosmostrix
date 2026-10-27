// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Fuzz target: the terminal escape hardening sink
//! (night-dragon-maintenance-5).
//!
//! [`escape_ctrl`] is the one gate between hostile text (config
//! values a shared or downloaded config file sets byte for byte) and
//! the rendered diagnostic line. Its contract is two properties, and
//! the harness asserts both on every input the fuzzer invents:
//!
//! 1. Terminal safety (the CWE-150 class): NO control character
//!    survives into the output except the newline, which is
//!    structural — the labeled-block renderer's line separator — not
//!    payload. An escape sequence that reaches the terminal can
//!    forge the diagnostic stream (OSC 52 clipboard writes, screen
//!    clears, DSR reply spam are the proved classes); a violation
//!    here is a fuzzer crash, libFuzzer saves the input, and CI
//!    fails.
//! 2. Honest-data fidelity: input carrying no control payload comes
//!    back byte-identical (the fast path's borrow contract — every
//!    normal error and warning text takes it).
//!
//! The `&str` type system refuses non-UTF-8 before this function
//! runs (the config file reader's `read_to_string` sits upstream),
//! so the harness drives the same UTF-8 gate the real callers sit
//! behind. The source is path-included, not copied (see
//! fuzz/Cargo.toml for the full rationale): the harness compiles the
//! same bytes the binary ships.

#![no_main]

#[path = "../../src/output/escape_ctrl.rs"]
mod escape_ctrl;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let out = escape_ctrl::escape_ctrl(text);

        assert!(
            !out.chars().any(|c| c != '\n' && c.is_control()),
            "terminal injection: escape_ctrl let a control character \
             through (input {text:?}, output {out:?})"
        );

        if !text.chars().any(|c| c != '\n' && c.is_control()) {
            assert_eq!(
                out, text,
                "escape_ctrl mutated honest data (input {text:?})"
            );
        }
    }
});
