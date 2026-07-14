// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for runtime scene cycling and transitions.
//!
//! v4.5.0 Phase 5 splits the original 959 LOC `tests_scene.rs` into focused
//! sub-modules for maintainability. No test behavior changed.

mod controls;
mod cycle;
mod fresh_entry;
mod residue;
mod sparse_entry;
mod transitions;

use super::Cloud;
use crate::frame::Frame;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

/// Create a monolith-style cloud for scene transition tests.
pub(crate) fn make_monolith_cloud() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        false,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Cosmos,
        RainStyle::Monolith,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(40, 20);
    cloud.scene_name = "monolith".to_string();
    cloud.clear_redraw_flags_for_test();
    cloud
}

/// Create a glyph (matrix) style cloud.
pub(crate) fn make_glyph_cloud() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        false,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(40, 20);
    cloud.scene_name = "matrix".to_string();
    cloud.clear_redraw_flags_for_test();
    cloud
}

/// Check if a frame has any dirty cells.
pub(crate) fn has_dirty_cells(frame: &Frame) -> bool {
    frame.is_dirty_all() || !frame.dirty_indices().is_empty()
}

// LOC guard — all scene split files must stay under 1000 LOC

/// All Rust source files must stay under 1000 LOC after the architecture split.
#[test]
fn all_rust_files_under_loc_cap() {
    let files = [
        "src/cloud/mod.rs",
        "src/cloud/spawn.rs",
        "src/cloud/tests/mod.rs",
        "src/cloud/tests/tests_scene/mod.rs",
        "src/cloud/tests/tests_scene/cycle.rs",
        "src/cloud/tests/tests_scene/transitions.rs",
        "src/cloud/tests/tests_scene/fresh_entry.rs",
        "src/cloud/tests/tests_scene/sparse_entry.rs",
        "src/cloud/tests/tests_scene/residue.rs",
        "src/cloud/tests/tests_scene/controls.rs",
        "src/cloud/tests/tests_visual_depth.rs",
        "src/cloud/tests/tests_monolith/mod.rs",
        "src/cloud/tests/tests_monolith/core.rs",
        "src/cloud/tests/tests_monolith/depth.rs",
        "src/cloud/tests/tests_monolith/residue.rs",
        "src/cloud/tests/tests_monolith/transitions.rs",
        "src/cloud/tests/tests_monolith/charset.rs",
        "src/cloud/tests/tests_architecture.rs",
        "src/cloud/scene_runtime.rs",
        "src/cloud/runtime_controls.rs",
    ];
    for path in &files {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let count = content.lines().count();
        assert!(count <= 1000, "{path}: {count} LOC exceeds 1000 cap");
    }
}

// Phase 4 guards (preserved from original tests_scene.rs)

/// The monolith mod.rs facade must stay small (under 200 LOC).
#[test]
fn phase4_monolith_facade_stays_small() {
    let content =
        std::fs::read_to_string("src/cloud/tests/tests_monolith/mod.rs").unwrap_or_default();
    let count = content.lines().count();
    assert!(
        count < 200,
        "monolith mod.rs facade should stay under 200 LOC (got {count})"
    );
}

/// All monolith split files must be under 1000 LOC.
#[test]
fn phase4_all_monolith_split_files_under_loc_cap() {
    let files = [
        "src/cloud/tests/tests_monolith/mod.rs",
        "src/cloud/tests/tests_monolith/core.rs",
        "src/cloud/tests/tests_monolith/depth.rs",
        "src/cloud/tests/tests_monolith/residue.rs",
        "src/cloud/tests/tests_monolith/transitions.rs",
        "src/cloud/tests/tests_monolith/charset.rs",
    ];
    for path in &files {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let count = content.lines().count();
        assert!(count <= 1000, "{path}: {count} LOC exceeds 1000 cap");
    }
}

/// All depth lab tests must still exist by name (string check on source).
#[test]
fn phase4_depth_lab_monolith_tests_still_exist() {
    let depth_file =
        std::fs::read_to_string("src/cloud/tests/tests_monolith/depth.rs").unwrap_or_default();
    let required_tests = [
        "depth_lab_monolith_sparse_lane_density_bounded_per_column",
        "depth_lab_monolith_empty_space_ratio_above_min_threshold",
        "depth_lab_monolith_no_full_height_continuous_wall",
        "depth_lab_monolith_bottom_residue_bounded_extended_rain",
    ];
    for test_name in &required_tests {
        assert!(
            depth_file.contains(test_name),
            "depth.rs must contain test '{test_name}'"
        );
    }
}

/// No monolith test coverage category was accidentally removed during split.
#[test]
fn phase4_no_monolith_coverage_category_removed() {
    let all_files: String = [
        "src/cloud/tests/tests_monolith/core.rs",
        "src/cloud/tests/tests_monolith/depth.rs",
        "src/cloud/tests/tests_monolith/residue.rs",
        "src/cloud/tests/tests_monolith/transitions.rs",
        "src/cloud/tests/tests_monolith/charset.rs",
    ]
    .iter()
    .map(|p| std::fs::read_to_string(p).unwrap_or_default())
    .collect();

    let required_categories: &[(&str, &str)] = &[
        ("sparse lane density", "sparse_lane_density"),
        ("empty-space ratio", "empty_space_ratio"),
        ("no full-height wall", "full_height"),
        ("bottom residue", "bottom_residue"),
        ("top clear", "top_cells_clear"),
        ("resize reset", "resize_reset"),
        ("charset transition", "charset_transition"),
        ("color/charset residue", "color_and_charset_transitions"),
    ];

    for (category, marker) in required_categories {
        assert!(
            all_files.contains(marker),
            "coverage category '{category}' (marker '{marker}') must exist in monolith test files"
        );
    }
}

// Phase 5 guards — scene split integrity

/// The scene mod.rs facade must stay small (under 300 LOC).
/// It contains shared helpers, the master LOC guard, Phase 4 preserved
/// guards, and Phase 5 scene split integrity guards.
#[test]
fn phase5_scene_facade_stays_small() {
    let content = std::fs::read_to_string("src/cloud/tests/tests_scene/mod.rs").unwrap_or_default();
    let count = content.lines().count();
    assert!(
        count < 300,
        "scene mod.rs facade should stay under 300 LOC (got {count})"
    );
}

/// All scene split files must be under 1000 LOC.
#[test]
fn phase5_all_scene_split_files_under_loc_cap() {
    let files = [
        "src/cloud/tests/tests_scene/mod.rs",
        "src/cloud/tests/tests_scene/cycle.rs",
        "src/cloud/tests/tests_scene/transitions.rs",
        "src/cloud/tests/tests_scene/fresh_entry.rs",
        "src/cloud/tests/tests_scene/sparse_entry.rs",
        "src/cloud/tests/tests_scene/residue.rs",
        "src/cloud/tests/tests_scene/controls.rs",
    ];
    for path in &files {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let count = content.lines().count();
        assert!(count <= 1000, "{path}: {count} LOC exceeds 1000 cap");
    }
}

/// No scene test coverage category was accidentally removed during split.
#[test]
fn phase5_no_scene_coverage_category_removed() {
    let all_files: String = [
        "src/cloud/tests/tests_scene/cycle.rs",
        "src/cloud/tests/tests_scene/transitions.rs",
        "src/cloud/tests/tests_scene/fresh_entry.rs",
        "src/cloud/tests/tests_scene/sparse_entry.rs",
        "src/cloud/tests/tests_scene/residue.rs",
        "src/cloud/tests/tests_scene/controls.rs",
    ]
    .iter()
    .map(|p| std::fs::read_to_string(p).unwrap_or_default())
    .collect();

    let required_categories: &[(&str, &str)] = &[
        ("scene cycle", "scene_cycle"),
        ("monolith to matrix", "monolith_to_matrix"),
        ("monolith to signal", "monolith_to_signal"),
        ("glyph to monolith", "glyph_to_monolith"),
        ("fresh entry", "fresh_entry"),
        ("sparse entry", "sparse_entry"),
        ("residue cleanup", "residue"),
        ("semantic invalidation", "semantic_invalidate"),
        ("speed/density controls", "speed_updates_after_scene_switch"),
        ("unknown scene guard", "unknown_scene_does_not_change_state"),
    ];

    for (category, marker) in required_categories {
        assert!(
            all_files.contains(marker),
            "coverage category '{category}' (marker '{marker}') must exist in scene test files"
        );
    }
}

/// No depth lab scene regression test was removed during split.
#[test]
fn phase5_depth_lab_scene_tests_still_exist() {
    let all_files: String = [
        "src/cloud/tests/tests_scene/residue.rs",
        "src/cloud/tests/tests_scene/transitions.rs",
    ]
    .iter()
    .map(|p| std::fs::read_to_string(p).unwrap_or_default())
    .collect();

    let required_tests = [
        "depth_lab_scene_switch_monolith_to_matrix_clears_phosphor",
        "depth_lab_scene_switch_monolith_to_signal_clears_drawn_cells",
        "depth_lab_scene_switch_glyph_to_monolith_renders_clean",
        "depth_lab_repeated_cycle_never_accumulates_residue",
    ];
    for test_name in &required_tests {
        assert!(
            all_files.contains(test_name),
            "scene tests must contain depth lab test '{test_name}'"
        );
    }
}
