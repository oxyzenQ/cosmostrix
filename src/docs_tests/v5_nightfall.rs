// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Static docs tests for v5.0.0 Nightfall product identity foundation.
//!
//! These tests verify that the roadmap, plan doc, and future vision doc
//! correctly establish v5.0.0 scope, boundaries, and release safety
//! without requiring a TTY or subprocess.

/// Roadmap doc.
const ROADMAP: &str = include_str!("../../docs/ROADMAP.md");

/// v5 Nightfall plan doc.
const V5_PLAN: &str = include_str!("../../docs/V5_NIGHTFALL_PLAN.md");

/// Future vision doc (exploratory).
const NEXT_VISION: &str = include_str!("../../docs/cosmostrix-next-vision.md");

/// Config source — for --show-preset documentation check.
const CONFIG_SRC: &str = include_str!("../config.rs");

/// Preset source — for print_show_preset existence check.
const PRESET_SRC: &str = include_str!("../preset.rs");

/// Example config file.
const EXAMPLE_CONFIG: &str = include_str!("../../config/cosmostrix.example.toml");

// ---------------------------------------------------------------------------
// Roadmap guards
// ---------------------------------------------------------------------------

#[test]
fn roadmap_has_v5_nightfall_active_section() {
    assert!(ROADMAP.contains("v5.0.0"), "roadmap must mention v5.0.0");
    assert!(
        ROADMAP.contains("Nightfall"),
        "roadmap must mention Nightfall codename"
    );
    assert!(
        ROADMAP.contains("Active Development"),
        "roadmap must have Active Development section"
    );
}

#[test]
fn roadmap_says_medium_major_release() {
    let lower = ROADMAP.to_lowercase();
    assert!(
        lower.contains("medium"),
        "roadmap must describe v5.0.0 as a medium major release"
    );
}

#[test]
fn roadmap_says_no_renderer_hot_path_rewrite() {
    let lower = ROADMAP.to_lowercase();
    assert!(
        lower.contains("no renderer hot-path rewrite")
            || ROADMAP.contains("no renderer hot-path rewrite"),
        "roadmap must state no renderer hot-path rewrite"
    );
}

#[test]
fn roadmap_says_no_50k_fps_promise() {
    assert!(
        ROADMAP.contains("50k") || ROADMAP.contains("50,000"),
        "roadmap must mention 50k and reject it as a promise"
    );
    assert!(
        ROADMAP.to_lowercase().contains("not a release promise")
            || ROADMAP.to_lowercase().contains("no 50k fps promise"),
        "roadmap must explicitly disavow 50k FPS promise"
    );
}

#[test]
fn roadmap_says_android_is_future_sibling_product() {
    let lower = ROADMAP.to_lowercase();
    assert!(
        lower.contains("android") && lower.contains("future sibling product"),
        "roadmap must state Android/Cosmostrix Live is future sibling product"
    );
}

// ---------------------------------------------------------------------------
// v5 Nightfall plan guards
// ---------------------------------------------------------------------------

#[test]
fn v5_plan_exists() {
    // The file was loaded via include_str! — if it didn't exist,
    // compilation would fail. This test is a named guard for clarity.
    assert!(
        !V5_PLAN.is_empty(),
        "V5_NIGHTFALL_PLAN.md must exist and be non-empty"
    );
}

#[test]
fn v5_plan_says_benchmark_update_before_tag() {
    let lower = V5_PLAN.to_lowercase();
    assert!(
        lower.contains("benchmark") && lower.contains("before") && lower.contains("tag"),
        "v5 plan must state release benchmark must be updated before tag"
    );
}

#[test]
fn v5_plan_says_terminal_writer_single_owner() {
    let lower = V5_PLAN.to_lowercase();
    assert!(
        lower.contains("single-owner") || lower.contains("single owner"),
        "v5 plan must state terminal writer remains single-owner"
    );
}

#[test]
fn v5_plan_says_benchmark_honesty_preserved() {
    let lower = V5_PLAN.to_lowercase();
    assert!(
        lower.contains("benchmark honesty"),
        "v5 plan must state benchmark honesty is preserved"
    );
}

#[test]
fn v5_plan_says_android_out_of_scope() {
    let lower = V5_PLAN.to_lowercase();
    assert!(
        lower.contains("android")
            && (lower.contains("out of scope") || lower.contains("no android code")),
        "v5 plan must state Android implementation is out of scope"
    );
}

// ---------------------------------------------------------------------------
// Future vision doc guards
// ---------------------------------------------------------------------------

#[test]
fn next_vision_says_exploratory() {
    let lower = NEXT_VISION.to_lowercase();
    assert!(
        lower.contains("exploratory"),
        "future vision doc must say this is exploratory"
    );
}

#[test]
fn next_vision_says_sibling_product() {
    let lower = NEXT_VISION.to_lowercase();
    assert!(
        lower.contains("sibling product") || lower.contains("sibling"),
        "future vision doc must describe Cosmostrix Live as sibling product"
    );
}

#[test]
fn next_vision_says_separate_repository() {
    let lower = NEXT_VISION.to_lowercase();
    assert!(
        lower.contains("separate repository")
            || lower.contains("own repository")
            || lower.contains("own codebase"),
        "future vision doc must say Cosmostrix Live is a separate repository"
    );
}

#[test]
fn next_vision_says_no_android_code_in_cli_repo() {
    let lower = NEXT_VISION.to_lowercase();
    assert!(
        lower.contains("no android code") || lower.contains("not part of"),
        "future vision doc must state no Android code in CLI repo"
    );
}

// ---------------------------------------------------------------------------
// Phase 2: Discoverability guards
// ---------------------------------------------------------------------------

#[test]
fn show_preset_documented_in_help_detail() {
    assert!(
        CONFIG_SRC.contains("--show-preset"),
        "config.rs must mention --show-preset in help-detail text"
    );
    assert!(
        CONFIG_SRC.contains("Show full details for a named preset"),
        "--show-preset must have a description in help-detail"
    );
}

#[test]
fn show_preset_impl_exists_in_preset_rs() {
    assert!(
        PRESET_SRC.contains("fn print_show_preset"),
        "preset.rs must contain print_show_preset function"
    );
}

#[test]
fn example_config_exists() {
    assert!(
        !EXAMPLE_CONFIG.is_empty(),
        "config/cosmostrix.example.toml must exist and be non-empty"
    );
}

#[test]
fn example_config_has_defaults_section() {
    assert!(
        EXAMPLE_CONFIG.contains("scene = monolith"),
        "example config must set scene = monolith"
    );
    assert!(
        EXAMPLE_CONFIG.contains("color = cosmos"),
        "example config must set color"
    );
    assert!(
        EXAMPLE_CONFIG.contains("fps = 60"),
        "example config must set fps"
    );
}

#[test]
fn example_config_has_profile_section() {
    assert!(
        EXAMPLE_CONFIG.contains("profile."),
        "example config must contain at least one profile.<name> section"
    );
    assert!(
        EXAMPLE_CONFIG.contains("profile.calm-night"),
        "example config must contain profile.calm-night"
    );
}

#[test]
fn referenced_docs_exist() {
    // These docs are referenced from --list-profiles and --dump-config.
    // They must exist (verified by include_str! compilation above),
    // but this named guard makes the intent explicit.
    let _ = include_str!("../../docs/PROFILE_EXAMPLES.md");
    let _ = include_str!("../../docs/ATMOSPHERE_PRESETS.md");
    let _ = include_str!("../../docs/PROFILE_ECOSYSTEM.md");
}

#[test]
fn v5_plan_has_phase_2() {
    assert!(V5_PLAN.contains("Phase 2"), "v5 plan must mention Phase 2");
    assert!(
        V5_PLAN.to_lowercase().contains("discoverab"),
        "v5 plan Phase 2 must mention discoverability"
    );
}

#[test]
fn profile_error_mentions_list_profiles() {
    let src = include_str!("../profile.rs");
    let lower = src.to_lowercase();
    assert!(
        lower.contains("--list-profiles"),
        "profile unknown error must hint --list-profiles"
    );
}

#[test]
fn preset_error_mentions_list_presets() {
    let src = include_str!("../preset.rs");
    let lower = src.to_lowercase();
    assert!(
        lower.contains("--list-presets"),
        "preset unknown error must hint --list-presets"
    );
}

// ---------------------------------------------------------------------------
// Phase 3: Cinematic breathing language guards
// ---------------------------------------------------------------------------

/// Cinematic breathing language doc.
const CINEMATIC_BREATHING: &str = include_str!("../../docs/CINEMATIC_BREATHING.md");

#[test]
fn breathing_doc_exists() {
    assert!(
        !CINEMATIC_BREATHING.is_empty(),
        "docs/CINEMATIC_BREATHING.md must exist and be non-empty"
    );
}

#[test]
fn breathing_doc_has_pacing_contract() {
    assert!(
        CINEMATIC_BREATHING.contains("Pacing Contract"),
        "breathing doc must contain 'Pacing Contract'"
    );
}

#[test]
fn breathing_doc_has_breathing_vocabulary() {
    assert!(
        CINEMATIC_BREATHING.contains("Breathing Vocabulary"),
        "breathing doc must contain 'Breathing Vocabulary'"
    );
}

#[test]
fn breathing_doc_defines_rest() {
    assert!(
        CINEMATIC_BREATHING.contains("baseline state"),
        "breathing doc must define Rest as the baseline state"
    );
}

#[test]
fn breathing_doc_defines_whisper() {
    let lower = CINEMATIC_BREATHING.to_lowercase();
    assert!(
        lower.contains("whisper") && CINEMATIC_BREATHING.contains("most subtle atmosphere effect"),
        "breathing doc must define Whisper as the most subtle effect"
    );
}

#[test]
fn breathing_doc_defines_compression() {
    let lower = CINEMATIC_BREATHING.to_lowercase();
    assert!(
        lower.contains("compression") && CINEMATIC_BREATHING.contains("visual field tightens"),
        "breathing doc must define Compression"
    );
}

#[test]
fn breathing_doc_defines_void() {
    let lower = CINEMATIC_BREATHING.to_lowercase();
    assert!(
        lower.contains("void") && CINEMATIC_BREATHING.contains("deliberate reduction"),
        "breathing doc must define Void"
    );
}

#[test]
fn breathing_doc_says_storm_never_default() {
    assert!(
        CINEMATIC_BREATHING.contains("Storm is never default"),
        "breathing doc must state Storm is never default"
    );
}

#[test]
fn breathing_doc_says_no_instant_transitions() {
    assert!(
        CINEMATIC_BREATHING.contains("No visual state change is instant"),
        "breathing doc must state no visual state change is instant"
    );
}

#[test]
fn breathing_doc_has_anti_patterns() {
    assert!(
        CINEMATIC_BREATHING.contains("Anti-patterns"),
        "breathing doc must contain Anti-patterns section"
    );
}

#[test]
fn breathing_doc_has_state_hierarchy() {
    assert!(
        CINEMATIC_BREATHING.contains("State Hierarchy"),
        "breathing doc must contain State Hierarchy section"
    );
}

#[test]
fn v5_plan_references_cinematic_breathing() {
    assert!(
        V5_PLAN.contains("CINEMATIC_BREATHING.md"),
        "v5 plan must reference CINEMATIC_BREATHING.md"
    );
}

#[test]
fn roadmap_references_breathing_language() {
    assert!(
        ROADMAP.contains("CINEMATIC_BREATHING.md")
            || ROADMAP.to_lowercase().contains("breathing language"),
        "roadmap must reference CINEMATIC_BREATHING.md or breathing language"
    );
}
