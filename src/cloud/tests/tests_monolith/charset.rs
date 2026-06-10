// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Monolith charset tests: charset transition, glyph style,
//! charset presets, color/charset transition residue.

use std::time::{Duration, Instant};

use super::{
    build_chars, charset_from_str, make_monolith_cloud, run_frames, visible_char_signature,
    visible_chars, Frame, CHARSET_TRANSITION_DURATION_MS,
};

#[test]
fn monolith_charset_transition_changes_segment_glyph_style() {
    let mut cloud = make_monolith_cloud(64, 24);
    let mut frame = Frame::new(64, 24, cloud.palette.bg);
    let start = Instant::now();

    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    cloud.rain_at(&mut frame, start);
    let before = visible_chars(&frame);
    assert!(before.iter().any(|&ch| ch == '0' || ch == '1'));

    frame.clear_dirty();
    cloud.transition_chars(vec!['A', 'B', 'C', 'D']);
    cloud.charset_transition_start =
        Some(start - Duration::from_millis(CHARSET_TRANSITION_DURATION_MS as u64 + 1));
    cloud.rain_at(&mut frame, start + Duration::from_millis(16));
    let after = visible_chars(&frame);

    assert!(
        !after.iter().any(|&ch| ch == '0' || ch == '1'),
        "code-like charset should remove binary segment accents"
    );
    assert_ne!(before, after);
}

#[test]
fn monolith_charset_cycles_produce_multiple_glyph_styles() {
    let mut cloud = make_monolith_cloud(80, 24);
    let mut frame = Frame::new(80, 24, cloud.palette.bg);
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;

    cloud.rain_at(&mut frame, start);
    let binary = visible_char_signature(&frame);

    let styles = [
        vec!['.', '-', '=', '+'],
        vec!['A', 'B', 'C', 'D'],
        vec!['\u{2580}', '\u{2584}', '\u{258C}', '\u{2590}'],
    ];
    let mut signatures = vec![binary];
    for (idx, chars) in styles.into_iter().enumerate() {
        frame.clear_dirty();
        cloud.transition_chars(chars);
        cloud.charset_transition_start =
            Some(start - Duration::from_millis(CHARSET_TRANSITION_DURATION_MS as u64 + 1));
        cloud.rain_at(
            &mut frame,
            start + Duration::from_millis(16 + idx as u64 * 16),
        );
        signatures.push(visible_char_signature(&frame));
    }
    signatures.sort();
    signatures.dedup();

    assert!(
        signatures.len() >= 3,
        "monolith charset cycling should produce at least three visible glyph styles"
    );
}

#[test]
fn monolith_charset_presets_drive_distinct_segment_glyphs() {
    let presets = [
        "binary",
        "matrix",
        "katakana",
        "code",
        "hacker",
        "cyberpunk",
    ];
    let mut signatures = Vec::new();

    for preset in presets {
        let charset = charset_from_str(preset, false).expect("known charset");
        let mut cloud = make_monolith_cloud(80, 24);
        cloud.init_chars(build_chars(charset, &[], false));
        cloud.reset(80, 24);
        cloud.clear_redraw_flags_for_test();

        let mut frame = Frame::new(80, 24, cloud.palette.bg);
        run_frames(&mut cloud, &mut frame, 8, 16);
        let signature = visible_char_signature(&frame);
        assert!(
            !signature.is_empty(),
            "{preset} should render monolith glyphs"
        );

        if preset == "binary" {
            assert!(signature.iter().any(|ch| matches!(ch, '0' | '1')));
        }
        if preset == "katakana" {
            assert!(signature
                .iter()
                .any(|ch| ('\u{FF66}'..='\u{FF9D}').contains(ch)));
        }
        signatures.push(signature);
    }

    signatures.sort();
    signatures.dedup();
    assert!(
        signatures.len() >= 5,
        "monolith should reflect real charset presets, got {} distinct styles",
        signatures.len()
    );
}
