// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! S-master-HUNT-8: live-reload charset transition parity tests.
//!
//! `start_transition_from_previous_charset` is the charset-subsystem mirror
//! of `start_transition_from_previous_palette`. It is called by the
//! live-reload rebuild path (`event_loop_config_rebuild`) when the charset
//! changes between config edits — the rebuild produces a fresh Cloud via
//! `create_cloud` (which calls `init_chars`, the instant path), and this
//! method re-arms the 500ms wave so the glyph swap is animated rather than
//! instant (parity with the 's'/'S' shortkey which uses `transition_chars`).
//!
//! These tests verify the function contract directly. The integration
//! (live-reload path actually invokes it) is covered by the gatekeeper
//! test suite's existing charset transition regression locks in
//! `tests/mod.rs` plus the field-level config diff trace assertions.

use super::make_cloud;

#[test]
fn start_transition_from_previous_charset_activates_wave() {
    let mut cloud = make_cloud();
    // Simulate the post-rebuild state: the new Cloud already has the NEW
    // chars loaded by init_chars inside create_cloud. The caller captured
    // the OLD shuffled char_pool before the swap.
    let old_pool = cloud.char_pool.clone();
    let new_chars = vec!['A', 'B', 'C', 'D'];
    cloud.init_chars(new_chars.clone());
    cloud.charset_transition_start = None;
    cloud.previous_char_pool.clear();
    cloud.force_draw_everything = false;
    cloud.semantic_invalidate = false;

    cloud.start_transition_from_previous_charset(old_pool.clone());

    // The wave is armed.
    assert!(cloud.charset_transition_start.is_some());
    // The OLD shuffled pool is installed as the transition source so the
    // shader can read both old (below wave) and new (above wave) glyphs.
    assert_eq!(cloud.previous_char_pool, old_pool);
    // The NEW canonical chars are untouched (the rebuild already installed
    // them via init_chars -> rebuild_char_pools -> self.chars = chars).
    assert_eq!(cloud.chars, new_chars);
    // The NEW char_pool is a shuffled pool of new_chars — it must contain
    // ONLY chars from new_chars (the rebuild re-shuffled it). We don't
    // assert exact equality because the shuffle is RNG-seeded.
    assert!(
        cloud.char_pool.iter().all(|ch| new_chars.contains(ch)),
        "new char_pool must contain only new-chars glyphs"
    );
    // Force redraw + semantic invalidate mirror transition_chars v18
    // cinematic unification so the wave is visible on every rain style.
    assert!(cloud.force_draw_everything);
    assert!(cloud.semantic_invalidate);
}

#[test]
fn start_transition_from_previous_charset_preserves_new_chars() {
    // The contract: start_transition_from_previous_charset ONLY seeds
    // previous_char_pool + arms the wave. It must NOT call
    // rebuild_char_pools (the rebuild already did that). If it did, the
    // new chars would be replaced by the OLD chars argument — a
    // regression that would silently undo the config edit.
    let mut cloud = make_cloud();
    let new_chars = vec!['X', 'Y', 'Z'];
    cloud.init_chars(new_chars.clone());

    cloud.start_transition_from_previous_charset(vec!['0', '1']);

    assert_eq!(
        cloud.chars, new_chars,
        "new canonical chars must be preserved — start_transition_from_previous_charset must NOT re-install the old chars"
    );
}

#[test]
fn start_transition_from_previous_charset_with_empty_prev_uses_binary_fallback() {
    // Defensive branch: if the caller passes an empty prev_chars (e.g. a
    // very first reload on a fresh session where char_pool was somehow
    // empty before the rebuild), the function falls back to the binary
    // default ['0', '1'] so the wave is visually meaningful rather than
    // empty. The caller's `!preserved_char_pool.is_empty()` guard usually
    // prevents this branch — but the function defends anyway, matching
    // transition_chars's own empty-pool fallback.
    let mut cloud = make_cloud();
    cloud.init_chars(vec!['A', 'B']);
    cloud.previous_char_pool.clear();

    cloud.start_transition_from_previous_charset(Vec::new());

    assert_eq!(cloud.previous_char_pool, vec!['0', '1']);
    assert!(cloud.charset_transition_start.is_some());
}
