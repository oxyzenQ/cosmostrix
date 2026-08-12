// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use super::*;

#[test]
fn none_is_ok() {
    // No --bench-scene flag → default lean path, no error.
    assert!(validate_bench_scene_str(None).is_ok());
}

#[test]
fn valid_lean() {
    assert!(validate_bench_scene_str(Some("lean")).is_ok());
}

#[test]
fn valid_production_draw() {
    assert!(validate_bench_scene_str(Some("production-draw")).is_ok());
}

#[test]
fn rejects_typo_leanax() {
    // Reported bug: "leanax" was silently accepted.
    let err = validate_bench_scene_str(Some("leanax")).unwrap_err();
    assert!(
        err.contains("invalid --bench-scene value 'leanax'"),
        "got: {err}"
    );
    assert!(
        err.contains("Valid scenes: lean, production-draw"),
        "got: {err}"
    );
}

#[test]
fn rejects_typo_axa() {
    // Reported bug: "axa" was silently accepted.
    let err = validate_bench_scene_str(Some("axa")).unwrap_err();
    assert!(
        err.contains("invalid --bench-scene value 'axa'"),
        "got: {err}"
    );
}

#[test]
fn rejects_typo_production_draw_garbage() {
    // Reported bug: "production-drawmadadadaxa" was silently accepted.
    let err = validate_bench_scene_str(Some("production-drawmadadadaxa")).unwrap_err();
    assert!(
        err.contains("invalid --bench-scene value 'production-drawmadadadaxa'"),
        "got: {err}"
    );
}

#[test]
fn rejects_empty_string() {
    let err = validate_bench_scene_str(Some("")).unwrap_err();
    assert!(err.contains("invalid --bench-scene value ''"), "got: {err}");
}

#[test]
fn rejects_case_variant() {
    // Strict: "Lean" (capitalized) is NOT valid.
    assert!(validate_bench_scene_str(Some("Lean")).is_err());
}

#[test]
fn rejects_production_draw_uppercase() {
    assert!(validate_bench_scene_str(Some("Production-Draw")).is_err());
}

#[test]
fn rejects_whitespace_padded() {
    assert!(validate_bench_scene_str(Some(" lean ")).is_err());
}

#[test]
fn error_message_lists_all_valid_scenes() {
    let err = validate_bench_scene_str(Some("bogus")).unwrap_err();
    for scene in VALID_BENCH_SCENES {
        assert!(err.contains(scene), "error msg missing '{scene}': {err}");
    }
}

#[test]
fn error_message_mentions_strict_contract() {
    let err = validate_bench_scene_str(Some("bogus")).unwrap_err();
    assert!(err.contains("strict"), "got: {err}");
    assert!(err.contains("not silently"), "got: {err}");
}
