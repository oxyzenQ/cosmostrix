// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Scene catalog tests — extracted from mod.rs.

#[allow(unused_imports)]
use super::*;

#[test]
fn cycle_scene_forward_order() {
    // Owner-pinned core trio: cinematic -> monolith -> matrix.
    assert_eq!(cycle_scene("cinematic", 1), "monolith");
    assert_eq!(cycle_scene("monolith", 1), "matrix");
    assert_eq!(cycle_scene("matrix", 1), "classic");
    // Tail of the cycle wraps back to the head.
    assert_eq!(cycle_scene("curiosity", 1), "cinematic");
}

#[test]
fn cycle_scene_backward_order() {
    // Core trio backward: matrix -> monolith -> cinematic.
    assert_eq!(cycle_scene("matrix", -1), "monolith");
    assert_eq!(cycle_scene("monolith", -1), "cinematic");
    // Head wraps backward to the tail.
    assert_eq!(cycle_scene("cinematic", -1), "curiosity");
}

#[test]
fn cycle_scene_unknown_returns_default() {
    assert_eq!(cycle_scene("nonexistent", 1), DEFAULT_SCENE);
    assert_eq!(cycle_scene("nonexistent", -1), DEFAULT_SCENE);
}

#[test]
fn cycle_scene_wraps_around() {
    // Double forward from matrix: matrix -> classic -> signal.
    assert_eq!(cycle_scene(cycle_scene("matrix", 1), 1), "signal");
    // Double backward from matrix: matrix -> monolith -> cinematic.
    assert_eq!(cycle_scene(cycle_scene("matrix", -1), -1), "cinematic");
    // Full lap forward returns to start.
    let mut cur = "cinematic";
    for _ in 0..SCENE_ORDER.len() {
        cur = cycle_scene(cur, 1);
    }
    assert_eq!(cur, "cinematic");
}

#[test]
fn scene_names_are_present() {
    assert_eq!(DEFAULT_SCENE, "cinematic");
    assert_eq!(
        all_scene_names(),
        &[
            "calm",
            "carbonic",
            "cinematic",
            "classic",
            "cosmic-dragon",
            "cosmos",
            "curiosity",
            "dragon-crystal",
            "hacker",
            "low-power",
            "matrix",
            "matrix_film",
            "monolith",
            "neon",
            "north-stars",
            "orange-cat",
            "signal",
            "storm",
        ]
    );
    for name in all_scene_names() {
        assert!(get_scene(name).is_some(), "missing scene {name}");
    }
}

#[test]
fn scene_catalog_has_eighteen_entries() {
    assert_eq!(SCENES.len(), 18, "catalog must contain 18 built-in scenes");
}

#[test]
fn matrix_film_scene_uses_film_accurate_values() {
    let s = get_scene("matrix_film").expect("matrix_film scene");
    assert_eq!(s.config.color, Some("neon-green"));
    assert_eq!(s.config.charset, Some("matrix"));
    assert_eq!(s.config.fps, Some(60.0));
    assert_eq!(s.config.speed, Some(22.0));
    assert_eq!(s.config.density, Some(0.85));
    assert_eq!(s.config.glitch_level, Some(GlitchLevel::Subtle));
    assert_eq!(s.config.rain_style, RainStyle::Glyph);
    // description must reference the 1999 cinematic homage so the
    // scene's purpose is self-documenting via --list-scenes / --show-scene.
    assert!(
        s.description.contains("1999"),
        "matrix_film description must reference 1999: {}",
        s.description
    );
}

#[test]
fn matrix_film_distinct_from_matrix_scene() {
    // matrix_film is the film-faithful homage; matrix is the modern
    // organic cascade. They must differ on density and speed so that
    // cycling into matrix_film is visually distinct from matrix.
    let matrix = get_scene("matrix").expect("matrix scene");
    let film = get_scene("matrix_film").expect("matrix_film scene");
    assert_ne!(
        matrix.config.density, film.config.density,
        "matrix_film must have different density than matrix"
    );
    assert_ne!(
        matrix.config.speed, film.config.speed,
        "matrix_film must have different speed than matrix"
    );
}

#[test]
fn cosmic_dragon_scene_marks_temporal_prediction_milestone() {
    let s = get_scene("cosmic-dragon").expect("cosmic-dragon scene");
    assert_eq!(s.config.color, Some("cosmos"));
    assert_eq!(s.config.charset, Some("binary"));
    assert_eq!(s.config.fps, Some(60.0));
    assert_eq!(s.config.speed, Some(12.0));
    assert_eq!(s.config.density, Some(0.65));
    assert_eq!(s.config.glitch_level, Some(GlitchLevel::Subtle));
    assert_eq!(s.config.rain_style, RainStyle::Glyph);
    // description must mention temporal-prediction milestone so the
    // scene's purpose is self-documenting via --list-scenes / --show-scene.
    assert!(
        s.description.contains("temporal-prediction"),
        "cosmic-dragon description must reference temporal-prediction: {}",
        s.description
    );
}

#[test]
fn scene_cycle_order_is_preserved() {
    // Owner-pinned first three (2026-08-24 directive) + full coverage.
    assert_eq!(&SCENE_ORDER[..3], &["cinematic", "monolith", "matrix"]);
    assert_eq!(
        SCENE_ORDER.len(),
        18,
        "all built-in scenes must be cyclable"
    );
    // Every SCENES entry must appear in SCENE_ORDER exactly once —
    // a new scene that forgets to join the cycle fails here.
    for s in SCENES {
        assert!(
            SCENE_ORDER.contains(&s.name),
            "scene '{}' missing from SCENE_ORDER",
            s.name
        );
    }
    let mut sorted = SCENE_ORDER.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        SCENE_ORDER.len(),
        "SCENE_ORDER has duplicates"
    );
}

#[test]
fn classic_scene_uses_cinematic_sparse_values() {
    let s = get_scene("classic").expect("classic scene");
    assert_eq!(s.config.color, Some("green"));
    assert_eq!(s.config.charset, Some("matrix"));
    assert_eq!(s.config.fps, Some(60.0));
    assert_eq!(s.config.speed, Some(12.0));
    assert_eq!(s.config.density, Some(0.70));
    assert_eq!(s.config.glitch_level, Some(GlitchLevel::Subtle));
    assert_eq!(s.config.rain_style, RainStyle::Glyph);
}

#[test]
fn storm_scene_uses_intense_cyberpunk_values() {
    let s = get_scene("storm").expect("storm scene");
    assert_eq!(s.config.color, Some("purple"));
    assert_eq!(s.config.charset, Some("cyberpunk"));
    assert_eq!(s.config.fps, Some(120.0));
    assert_eq!(s.config.speed, Some(28.0));
    assert_eq!(s.config.density, Some(1.10));
    assert_eq!(s.config.glitch_level, Some(GlitchLevel::Intense));
}

#[test]
fn low_power_scene_caps_fps_at_thirty() {
    let s = get_scene("low-power").expect("low-power scene");
    assert_eq!(s.config.fps, Some(30.0));
    assert_eq!(s.config.speed, Some(5.0));
    assert_eq!(s.config.density, Some(0.45));
    assert_eq!(s.config.color, Some("green"));
    assert_eq!(s.config.charset, Some("binary"));
    assert_eq!(s.config.glitch_level, Some(GlitchLevel::None));
}

#[test]
fn hacker_scene_uses_hacker_charset() {
    let s = get_scene("hacker").expect("hacker scene");
    assert_eq!(s.config.charset, Some("hacker"));
    assert_eq!(s.config.speed, Some(24.0));
    assert_eq!(s.config.density, Some(0.95));
}

#[test]
fn calm_scene_uses_ocean_zen_density() {
    let s = get_scene("calm").expect("calm scene");
    assert_eq!(s.config.color, Some("ocean"));
    assert_eq!(s.config.charset, Some("minimal"));
    assert_eq!(s.config.speed, Some(6.0));
    assert_eq!(s.config.density, Some(0.40));
    assert_eq!(s.config.glitch_level, Some(GlitchLevel::None));
}

#[test]
fn scenes_select_expected_rain_style() {
    assert_eq!(rain_style_for_scene("matrix"), Some(RainStyle::Glyph));
    assert_eq!(rain_style_for_scene("signal"), Some(RainStyle::Glyph));
    assert_eq!(rain_style_for_scene("monolith"), Some(RainStyle::Monolith));
}

#[test]
fn monolith_scene_uses_premium_motion_defaults() {
    let monolith = get_scene("monolith").expect("monolith scene");
    assert_eq!(monolith.config.speed, Some(30.0));
    assert_eq!(monolith.config.density, Some(0.85));
    assert_eq!(monolith.config.charset, Some("zen"));
}

#[test]
fn scene_parser_is_case_insensitive() {
    assert_eq!(validate_scene_name("SIGNAL").unwrap(), "signal");
    assert_eq!(validate_scene_name(" Monolith ").unwrap(), "monolith");
}

#[test]
fn invalid_scene_error_mentions_discovery() {
    let err = validate_scene_name("nonexistent").unwrap_err();
    assert!(
        err.contains("error: unknown scene"),
        "scene error must use 'unknown' terminology: {err}"
    );
    assert!(
        err.contains("--list-scenes"),
        "scene error must reference --list-scenes: {err}"
    );
}

#[test]
fn list_scenes_output_includes_all_scenes() {
    let text = list_scenes_text();
    for name in all_scene_names() {
        assert!(text.contains(name), "missing scene {name}");
    }
    assert!(
        text.contains("low-power"),
        "list must include low-power scene"
    );
    assert!(text.contains("storm"), "list must include storm scene");
}

#[test]
fn show_scene_text_includes_header_and_usage() {
    let info = get_scene("storm").expect("storm scene");
    let text = show_scene_text(info);
    assert!(text.starts_with("SCENE: storm"), "header missing: {text}");
    assert!(
        text.contains("Description:"),
        "description label missing: {text}"
    );
    assert!(
        text.contains("Configuration:"),
        "config label missing: {text}"
    );
    assert!(
        text.contains("color        = purple"),
        "color field missing: {text}"
    );
    assert!(
        text.contains("fps          = 120"),
        "fps field missing: {text}"
    );
    assert!(
        text.contains("rain-style   = glyph"),
        "rain-style missing: {text}"
    );
    assert!(
        text.contains("cosmostrix --scene storm"),
        "usage hint missing: {text}"
    );
}

#[test]
fn show_scene_text_handles_partial_scene() {
    // v18: all scenes now set all fields. This test verifies that
    // show_scene_text correctly renders a fully-populated scene.
    let info = get_scene("matrix").expect("matrix scene");
    let text = show_scene_text(info);
    assert!(text.contains("SCENE: matrix"), "header missing: {text}");
    assert!(
        text.contains("rain-style   = glyph"),
        "rain-style missing: {text}"
    );
    // v18: matrix now sets color, charset, fps, speed, density
    assert!(
        text.contains("color        = neon-green"),
        "color field missing: {text}"
    );
    assert!(
        text.contains("charset      = matrix"),
        "charset field missing: {text}"
    );
    assert!(
        text.contains("fps          = 60"),
        "fps field missing: {text}"
    );
}

// Submodules (moved from src/ root for clean src/ layout)
