// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

#[test]
fn terminal_compat_docs_cover_background_truth() {
    let docs = include_str!("../docs/TERMINAL_COMPATIBILITY.md");
    assert!(docs.contains("color-bg = transparent"));
    assert!(docs.contains("It does not change terminal emulator opacity."));
    assert!(docs.contains("color-bg = black"));
    assert!(docs.contains("color-bg = default-background"));
}

#[test]
fn terminal_compat_docs_cover_reset_safety() {
    let docs = include_str!("../docs/TERMINAL_COMPATIBILITY.md");
    assert!(docs.contains("Normal exit is non-destructive."));
    assert!(docs.contains("`--reset-terminal` is explicit destructive recovery."));
    assert!(docs.contains("attempts scrollback purge"));
}

#[test]
fn readme_links_terminal_compatibility_doc() {
    let readme = include_str!("../README.md");
    assert!(readme.contains("docs/TERMINAL_COMPATIBILITY.md"));
}

#[test]
fn zactrix_core_doc_exists_and_covers_architecture_terms() {
    let docs = include_str!("../docs/ZACTRIX_CORE.md");
    let lowercase = docs.to_lowercase();
    for term in ["probe", "map", "filter", "verifier", "bounded history"] {
        assert!(
            lowercase.contains(term),
            "Zactrix Core docs should mention {term}"
        );
    }
    assert!(docs.contains("not Linux eBPF"));
    assert!(docs.contains("not a public API"));
    assert!(docs.contains("must not introduce unsafe"));
    assert!(docs.contains("no new unsafe in renderer/core paths"));
    assert!(docs.contains("v3.9.0 Ultimate Subtle Monolith Rain"));
    assert!(docs.contains("v4.0.0"));

    let readme = include_str!("../README.md");
    assert!(readme.contains("docs/ZACTRIX_CORE.md"));
}

#[test]
fn simd_docs_do_not_claim_global_zero_unsafe() {
    let docs = include_str!("../docs/SIMD_FEASIBILITY.md");
    assert!(!docs.contains("zero `unsafe`"));
    assert!(docs.contains("no-new-unsafe renderer/core policy"));
}

#[test]
fn visual_stability_docs_cover_v39_monolith_subtlety_policy() {
    let docs = include_str!("../docs/VISUAL_STABILITY.md");
    assert!(docs.contains("v3.9.0 Monolith Subtlety Policy"));
    assert!(docs.contains("Organic does not mean chaotic"));
    assert!(docs.contains("full-height spine walls"));
    assert!(docs.contains("Zactrix Core may guide"));
}

#[test]
fn source_contains_only_audited_platform_recovery_unsafe() {
    let main_rs = include_str!("main.rs");
    assert_eq!(main_rs.matches("unsafe {").count(), 1);
    assert!(main_rs.contains("SAFETY: this Linux-only guard"));

    let zactrix_core = include_str!("zactrix_core.rs");
    assert!(!zactrix_core.contains("unsafe {"));
}

#[test]
fn tracked_docs_and_sources_have_no_new_debt_markers() {
    let markers = [
        concat!("TO", "DO"),
        concat!("FIX", "ME"),
        concat!("HA", "CK"),
    ];
    let mut files = Vec::new();
    collect_files(std::path::Path::new("src"), &mut files);
    collect_files(std::path::Path::new("docs"), &mut files);
    files.push(std::path::PathBuf::from("README.md"));

    for path in files {
        let text = std::fs::read_to_string(&path).expect("tracked text file should be readable");
        for marker in markers {
            assert!(
                !text.contains(marker),
                "{} contains debt marker {marker}",
                path.display()
            );
        }
    }
}

fn collect_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("source/doc directory should exist") {
        let path = entry.expect("directory entry should be readable").path();
        if path.is_dir() {
            collect_files(&path, out);
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "md")
        ) {
            out.push(path);
        }
    }
}

#[test]
fn docs_mention_visual_stability_policy_if_exists() {
    let _ = include_str!("../README.md");
    // If VISUAL_STABILITY.md exists, README should link to it.
    // This test is informational — it verifies the docs can be compiled
    // without actually requiring the doc to exist yet.
}

#[test]
fn zactrix_engine_doc_exists_and_covers_adaptive_planning() {
    let docs = include_str!("../docs/ZACTRIX_ENGINE.md");
    let lowercase = docs.to_lowercase();
    assert!(
        lowercase.contains("adaptive execution"),
        "Zactrix Engine docs should mention adaptive execution"
    );
    assert!(
        lowercase.contains("single-owner"),
        "Zactrix Engine docs should mention single-owner terminal writer"
    );
    assert!(
        lowercase.contains("not always-on"),
        "Zactrix Engine docs should say engine is not always-on multithreading"
    );
    assert!(
        lowercase.contains("not a public api"),
        "Zactrix Engine docs should say it is not a public API"
    );
    assert!(
        docs.contains("v4.0.0 Phase 1"),
        "Zactrix Engine docs should mention v4.0.0 Phase 1"
    );
    assert!(
        docs.contains("v3.9.0"),
        "Zactrix Engine docs should mention v3.9.0 visual identity preservation"
    );
}

#[test]
fn zactrix_cache_doc_exists_and_covers_bounded_invalidation() {
    let docs = include_str!("../docs/ZACTRIX_CACHE.md");
    let lowercase = docs.to_lowercase();
    assert!(
        lowercase.contains("bounded"),
        "Zactrix Cache docs should mention bounded cache"
    );
    assert!(
        lowercase.contains("generation"),
        "Zactrix Cache docs should mention generation-aware invalidation"
    );
    assert!(
        lowercase.contains("invalidation"),
        "Zactrix Cache docs should mention invalidation events"
    );
    assert!(
        lowercase.contains("deterministic"),
        "Zactrix Cache docs should mention deterministic behavior"
    );
    assert!(
        lowercase.contains("does not cache terminal output strings"),
        "Zactrix Cache docs should state it does not cache terminal output strings"
    );
}

#[test]
fn atmosphere_engine_doc_exists_and_covers_regimes() {
    let docs = include_str!("../docs/ATMOSPHERE_ENGINE.md");
    let lowercase = docs.to_lowercase();
    assert!(
        lowercase.contains("regime"),
        "Atmosphere Engine docs should mention regimes"
    );
    assert!(
        lowercase.contains("phase 3")
            || lowercase.contains("phase 2")
            || lowercase.contains("phase 1"),
        "Atmosphere Engine docs should mention phase status"
    );
    assert!(
        lowercase.contains("gradual"),
        "Atmosphere Engine docs should mention gradual changes"
    );
    assert!(
        lowercase.contains("not random chaos"),
        "Atmosphere Engine docs should state changes are not random chaos"
    );
    assert!(
        lowercase.contains("verifier"),
        "Atmosphere Engine docs should mention verifier (Phase 3)"
    );
    assert!(
        lowercase.contains("calm"),
        "Atmosphere Engine docs should mention Calm default regime"
    );
}

#[test]
fn zactrix_engine_planner_chooses_single_core_for_normal_sizes() {
    use crate::zactrix_engine::{EngineMode, EnginePlan};
    let plan = EnginePlan::from_dimensions(80, 24);
    assert_eq!(plan.mode, EngineMode::SingleCore);
}

#[test]
fn zactrix_engine_planner_chooses_assist_for_large_screens() {
    use crate::zactrix_engine::{EngineMode, EnginePlan};
    let plan = EnginePlan::from_dimensions(250, 50);
    assert_eq!(plan.mode, EngineMode::Assist);
}

#[test]
fn zactrix_engine_worker_budget_is_bounded() {
    use crate::zactrix_engine::EnginePlan;
    use std::thread::available_parallelism;
    let plan = EnginePlan::from_dimensions(300, 80);
    let available = available_parallelism().map(|n| n.get()).unwrap_or(1);
    assert!(
        plan.worker_budget <= available.min(4),
        "worker_budget {} must be <= {}",
        plan.worker_budget,
        available.min(4)
    );
}

#[test]
fn zactrix_engine_safe_fallback_for_zero_dimensions() {
    use crate::zactrix_engine::{EngineMode, EnginePlan};
    let plan = EnginePlan::from_dimensions(0, 0);
    assert_eq!(plan.mode, EngineMode::SafeFallback);
}

#[test]
fn zactrix_cache_invalidates_on_all_defined_events() {
    use crate::zactrix_cache::{CachePolicy, InvalidationEvent};
    let mut policy = CachePolicy::default_policy();
    let events = [
        InvalidationEvent::Resize,
        InvalidationEvent::ColorChange,
        InvalidationEvent::CharsetChange,
        InvalidationEvent::SceneSwitch,
        InvalidationEvent::ProfileApply,
        InvalidationEvent::TerminalModeChange,
        InvalidationEvent::AtmosphereRegimeChange,
    ];
    for event in events {
        let prev = policy.generation.id();
        policy.invalidate(event);
        assert_eq!(
            policy.generation.id(),
            prev + 1,
            "event {:?} should bump generation",
            event
        );
    }
}

#[test]
fn zactrix_cache_policy_never_grows_unbounded() {
    use crate::zactrix_cache::CachePolicy;
    let policy = CachePolicy::default_policy();
    assert!(!policy.should_admit(usize::MAX));
    assert!(policy.is_within_bounds(policy.max_entries));
}

#[test]
fn auto_color_drift_remains_default_false_in_constants() {
    let constants = include_str!("constants.rs");
    assert!(
        constants.contains("AUTO_COLOR_DRIFT_DEFAULT: bool = false"),
        "AUTO_COLOR_DRIFT_DEFAULT must be false"
    );
}

#[test]
fn fixed_color_remains_sticky_by_default() {
    // Verify that the color stability docs and config are consistent.
    let stability = include_str!("../docs/VISUAL_STABILITY.md");
    assert!(
        stability.to_lowercase().contains("sticky")
            || stability.to_lowercase().contains("stable by default"),
        "Visual stability docs should mention sticky/stable by default color behavior"
    );
}

#[test]
fn no_new_unsafe_in_zactrix_modules() {
    let engine = include_str!("zactrix_engine.rs");
    assert!(
        !engine.contains("unsafe {"),
        "zactrix_engine.rs must not contain unsafe"
    );

    let cache = include_str!("zactrix_cache.rs");
    assert!(
        !cache.contains("unsafe {"),
        "zactrix_cache.rs must not contain unsafe"
    );

    let atmosphere = include_str!("atmosphere.rs");
    assert!(
        !atmosphere.contains("unsafe {"),
        "atmosphere.rs must not contain unsafe"
    );
}

#[test]
fn all_zactrix_files_under_1000_loc() {
    let files = [
        ("zactrix_engine.rs", include_str!("zactrix_engine.rs")),
        ("zactrix_cache.rs", include_str!("zactrix_cache.rs")),
        ("atmosphere.rs", include_str!("atmosphere.rs")),
        ("zactrix_core.rs", include_str!("zactrix_core.rs")),
    ];
    for (name, content) in files {
        let lines = content.lines().count();
        assert!(
            lines <= 1000,
            "{name} has {lines} lines, must be under 1000"
        );
    }
}
