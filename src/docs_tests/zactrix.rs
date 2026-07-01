// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Zactrix Engine/Cache/Core doc guards, planner tests, and architecture split guards.
//!
//! v5.0.4: Cosmostrix is single-thread by design. All parallelism scaffolding removed.
//! Terminal writer is single-owner at all times. Immutable invariant.

#[test]
fn zactrix_engine_doc_exists_and_covers_adaptive_planning() {
    let docs = include_str!("../../docs/ZACTRIX_ENGINE.md");
    let lowercase = docs.to_lowercase();
    assert!(
        lowercase.contains("single-owner"),
        "Zactrix Engine docs should mention single-owner terminal writer"
    );
    assert!(
        lowercase.contains("not a public api"),
        "Zactrix Engine docs should say it is not a public API"
    );
}

#[test]
fn zactrix_cache_doc_exists_and_covers_bounded_invalidation() {
    let docs = include_str!("../../docs/ZACTRIX_CACHE.md");
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
}

#[test]
fn atmosphere_engine_doc_exists_and_covers_regimes() {
    let docs = include_str!("../../docs/ATMOSPHERE_ENGINE.md");
    let lowercase = docs.to_lowercase();
    assert!(
        lowercase.contains("regime"),
        "Atmosphere Engine docs should mention regimes"
    );
    assert!(
        lowercase.contains("calm"),
        "Atmosphere Engine docs should mention Calm default regime"
    );
}

// ── Zactrix Engine planner / cache functional guards ─────────────────────

#[test]
fn zactrix_engine_planner_chooses_single_core_for_all_sizes() {
    use crate::zactrix_engine::{EngineMode, EnginePlan};
    // All terminal sizes get SingleCore — cosmostrix is single-thread.
    for (cols, rows) in [(80, 24), (250, 50), (300, 80)] {
        let plan = EnginePlan::from_dimensions(cols, rows);
        assert_eq!(plan.mode, EngineMode::SingleCore);
    }
}

#[test]
fn zactrix_engine_worker_budget_is_always_zero() {
    use crate::zactrix_engine::EnginePlan;
    for (cols, rows) in [(80, 24), (300, 80)] {
        let plan = EnginePlan::from_dimensions(cols, rows);
        assert_eq!(
            plan.worker_budget, 0,
            "worker_budget must be 0 for single-thread"
        );
    }
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

// ── v5.0.4: Single-thread architecture guard tests ───────────────────

#[test]
fn zactrix_engine_facade_reexports_compile() {
    use crate::zactrix_engine::{
        EngineMode, EnginePlan, EngineProbe, IdlePolicy, RenderPlan, RuntimeMode,
        TerminalWriterPolicy, ZactrixSystemConfig,
    };
    let _mode: EngineMode = EngineMode::SingleCore;
    let _plan: EnginePlan = EnginePlan::from_dimensions(80, 24);
    let _config = ZactrixSystemConfig::default();
    let _render: RenderPlan = RenderPlan::default();
    let _policy: TerminalWriterPolicy = TerminalWriterPolicy::default();
    let _runtime: RuntimeMode = RuntimeMode::default();
    let _idle: IdlePolicy = IdlePolicy::default();
    let _probe = EngineProbe::from_dimensions(120, 40);
    let _ = (
        _mode, _plan, _config, _render, _policy, _runtime, _idle, _probe,
    );
}

#[test]
fn zactrix_engine_terminal_writer_label_is_single_owner() {
    use crate::zactrix_engine::{EnginePlan, TerminalWriterPolicy};
    let plan = EnginePlan::from_dimensions(80, 24);
    assert_eq!(TerminalWriterPolicy::default().as_str(), "single-owner");
    assert!(plan.terminal_writer_single_owner);
}

#[test]
fn zactrix_engine_runtime_mode_labels_are_stable() {
    use crate::zactrix_engine::RuntimeMode;
    assert!(!RuntimeMode::Calm.as_str().is_empty());
    assert!(!RuntimeMode::Normal.as_str().is_empty());
    assert!(!RuntimeMode::Stress.as_str().is_empty());
}

#[test]
fn zactrix_engine_idle_policy_default_is_adaptive_sleep() {
    use crate::zactrix_engine::IdlePolicy;
    assert_eq!(IdlePolicy::default().as_str(), "adaptive-sleep");
}

#[test]
fn zactrix_engine_render_plan_default_is_single_owner() {
    use crate::zactrix_engine::RenderPlan;
    let plan = RenderPlan::default();
    assert_eq!(plan.writer_policy.as_str(), "single-owner");
}

#[test]
fn zactrix_engine_docs_mention_single_thread() {
    let docs = include_str!("../../docs/ZACTRIX_ENGINE.md");
    let lower = docs.to_lowercase();
    assert!(
        lower.contains("single-owner"),
        "ZACTRIX_ENGINE.md must mention single-owner"
    );
}

#[test]
fn zactrix_engine_docs_say_no_real_parallel_terminal_writing() {
    let docs = include_str!("../../docs/ZACTRIX_ENGINE.md");
    assert!(
        docs.contains("never parallelize terminal writes"),
        "ZACTRIX_ENGINE.md must say never parallelize terminal writes"
    );
}

// ── Zactrix System diagnostics guards ──────────────────────────────────

#[test]
fn zactrix_system_runtime_mode_default_is_normal() {
    use crate::zactrix_engine::{RuntimeMode, ZactrixSystemConfig};
    let sys = ZactrixSystemConfig::default();
    assert_eq!(sys.runtime_mode, RuntimeMode::Normal);
    assert_eq!(sys.runtime_mode.as_str(), "normal");
}

#[test]
fn zactrix_system_render_plan_default_is_single_owner() {
    use crate::zactrix_engine::{RenderPlan, TerminalWriterPolicy};
    let render = RenderPlan::default();
    assert_eq!(render.writer_policy, TerminalWriterPolicy::SingleOwner);
    assert_eq!(render.writer_policy.as_str(), "single-owner");
}

#[test]
fn zactrix_system_idle_policy_default_is_adaptive_sleep() {
    use crate::zactrix_engine::{IdlePolicy, ZactrixSystemConfig};
    let sys = ZactrixSystemConfig::default();
    assert_eq!(sys.idle_policy, IdlePolicy::AdaptiveSleep);
    assert_eq!(sys.idle_policy.as_str(), "adaptive-sleep");
}

#[test]
fn zactrix_system_info_emits_zactrix_system_section() {
    let main_rs = include_str!("../main.rs");
    assert!(
        main_rs.contains("ZACTRIX SYSTEM"),
        "main.rs must emit ZACTRIX SYSTEM section in -i output"
    );
    assert!(
        main_rs.contains("runtime_mode"),
        "main.rs must emit runtime_mode field"
    );
    assert!(
        main_rs.contains("single-thread"),
        "main.rs must state single-thread architecture"
    );
    assert!(
        main_rs.contains("render_plan"),
        "main.rs must emit render_plan field"
    );
    assert!(
        main_rs.contains("idle_policy"),
        "main.rs must emit idle_policy field"
    );
}

#[test]
fn zactrix_system_benchmark_emits_zactrix_system_section() {
    let bench = include_str!("../bench_report.rs");
    assert!(
        bench.contains("ZACTRIX SYSTEM"),
        "bench_report.rs must emit ZACTRIX SYSTEM section"
    );
    assert!(
        bench.contains("runtime_mode"),
        "bench_report.rs must emit runtime_mode field"
    );
    assert!(
        bench.contains("render_plan"),
        "bench_report.rs must emit render_plan field"
    );
    assert!(
        bench.contains("idle_policy"),
        "bench_report.rs must emit idle_policy field"
    );
}

#[test]
fn zactrix_system_existing_zactrix_engine_benchmark_unchanged() {
    let bench = include_str!("../bench_report.rs");
    assert!(
        bench.contains("ZACTRIX ENGINE"),
        "bench_report.rs must still have ZACTRIX ENGINE section"
    );
    assert!(
        bench.contains("actual_execution"),
        "ZACTRIX ENGINE must still have actual_execution"
    );
    assert!(
        bench.contains("terminal_writer"),
        "ZACTRIX ENGINE must still have terminal_writer"
    );
}

// ── Depth Regression diagnostics guards ─────────────────────────────────

#[test]
fn depth_lab_benchmark_actual_execution_is_single_threaded() {
    let bench = include_str!("../bench_report.rs");
    assert!(
        bench.contains("\"single-threaded-renderer\""),
        "bench_report.rs must emit actual_execution: single-threaded-renderer"
    );
}

#[test]
fn depth_lab_benchmark_terminal_writer_is_single_owner() {
    let bench = include_str!("../bench_report.rs");
    assert!(
        bench.contains("\"single-owner\""),
        "bench_report.rs must emit terminal_writer: single-owner"
    );
}

#[test]
fn depth_lab_benchmark_render_plan_remains_single_owner() {
    use crate::zactrix_engine::{RenderPlan, TerminalWriterPolicy};
    let render = RenderPlan::default();
    assert_eq!(render.writer_policy, TerminalWriterPolicy::SingleOwner);
}

#[test]
fn depth_lab_no_active_parallel_compute_claimed() {
    use crate::zactrix_engine::EngineMode;
    // Only SingleCore and SafeFallback exist — no parallel variants.
    let _modes = [EngineMode::SingleCore, EngineMode::SafeFallback];
}

#[test]
fn depth_lab_info_output_zactrix_system_honest() {
    let main_rs = include_str!("../main.rs");
    let required_fields = [
        "runtime_mode",
        "render_plan",
        "idle_policy",
        "single-thread",
    ];
    for field in &required_fields {
        assert!(
            main_rs.contains(field),
            "main.rs -i output must include '{}'",
            field
        );
    }
}

#[test]
fn depth_lab_visual_stability_doc_exists() {
    let docs = include_str!("../../docs/VISUAL_STABILITY.md");
    let lower = docs.to_lowercase();
    assert!(
        lower.contains("depth regression"),
        "VISUAL_STABILITY.md must mention depth regression"
    );
    assert!(
        lower.contains("v4.0.1"),
        "VISUAL_STABILITY.md must reference v4.0.1 visual identity"
    );
}

#[test]
fn depth_lab_visual_stability_doc_mentions_zactrix_guard() {
    let docs = include_str!("../../docs/VISUAL_STABILITY.md");
    assert!(
        docs.contains("single-owner"),
        "VISUAL_STABILITY.md must mention single-owner terminal writer"
    );
}

// ── v4.5.0 Phase 6: Closure prep docs guards ──────────────────────────

#[test]
fn phase6_roadmap_doc_exists() {
    let docs = include_str!("../../docs/ROADMAP.md");
    assert!(
        !docs.is_empty(),
        "docs/ROADMAP.md must exist and be non-empty"
    );
}

#[test]
fn phase6_roadmap_mentions_future_versions() {
    let docs = include_str!("../../docs/ROADMAP.md");
    assert!(docs.contains("v4.6"), "ROADMAP.md must mention v4.6");
    assert!(docs.contains("v5.0"), "ROADMAP.md must mention v5.0");
}

#[test]
fn phase6_roadmap_says_terminal_writer_single_owner() {
    let docs = include_str!("../../docs/ROADMAP.md");
    let lower = docs.to_lowercase();
    assert!(
        lower.contains("terminal writer") && lower.contains("single-owner"),
        "ROADMAP.md must say terminal writer remains single-owner"
    );
}

#[test]
fn phase6_all_docs_test_modules_under_loc_cap() {
    let files = [
        "src/docs_tests/mod.rs",
        "src/docs_tests/assets.rs",
        "src/docs_tests/endurance.rs",
        "src/docs_tests/metadata.rs",
        "src/docs_tests/readme.rs",
        "src/docs_tests/release.rs",
        "src/docs_tests/safety.rs",
        "src/docs_tests/zactrix.rs",
    ];
    for path in &files {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let count = content.lines().count();
        assert!(count <= 1000, "{path}: {count} LOC exceeds 1000 cap");
    }
}

// v4.6-v4.8 era doc guards kept for backward compatibility

#[test]
fn v46_expansion_doc_single_owner_invariant() {
    let docs = include_str!("../../docs/ATMOSPHERE_EXPANSION.md");
    let lower = docs.to_lowercase();
    assert!(
        lower.contains("single-owner"),
        "ATMOSPHERE_EXPANSION.md must mention single-owner invariant"
    );
}

#[test]
fn v46_expansion_doc_no_parallel_compute() {
    let docs = include_str!("../../docs/ATMOSPHERE_EXPANSION.md");
    assert!(
        docs.contains("compute_parallelism: disabled"),
        "ATMOSPHERE_EXPANSION.md must state compute_parallelism is disabled"
    );
}

#[test]
fn v48_benchmark_docs_state_single_owner() {
    let docs = include_str!("../../benchmark/README.md");
    assert!(
        docs.contains("single-owner"),
        "benchmark/README.md must state terminal_writer single-owner"
    );
}

#[test]
fn v48_benchmark_docs_state_compute_parallelism_disabled() {
    let docs = include_str!("../../benchmark/README.md");
    assert!(
        docs.contains("compute_parallelism") && docs.contains("disabled"),
        "benchmark/README.md must state compute_parallelism disabled"
    );
}

// ── v4.8.0 Phase 2A: Color pipeline optimization guards ─────────────────

#[test]
fn v48_phase2a_set_force_is_single_threaded() {
    let frame_rs = include_str!("../frame.rs");
    assert!(
        frame_rs.contains("set_force"),
        "frame.rs must have set_force optimization"
    );
    assert!(
        !frame_rs.contains("std::sync::atomic"),
        "frame.rs must not use atomics (no parallel access)"
    );
}
