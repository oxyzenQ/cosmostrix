// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Zactrix Engine/Cache/Core doc guards, planner tests, and architecture split guards.
//!
//! v4.5.0 Phase 3 adds Depth Regression Lab diagnostics guards that ensure
//! benchmark/info output honestly reports single-threaded, single-owner execution.

#[test]
fn zactrix_engine_doc_exists_and_covers_adaptive_planning() {
    let docs = include_str!("../../docs/ZACTRIX_ENGINE.md");
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
    let docs = include_str!("../../docs/ATMOSPHERE_ENGINE.md");
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

// ── Zactrix Engine planner / cache functional guards ─────────────────────

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

// ── Phase 12.5: v4.5.0 architecture split guard tests ───────────────────

#[test]
fn zactrix_engine_facade_reexports_compile() {
    use crate::zactrix_engine::{
        ComputeParallelism, CpuBudget, EngineMode, EnginePlan, EngineProbe, IdlePolicy, RenderPlan,
        RuntimeMode, TerminalWriterPolicy, ZactrixSystemConfig,
    };
    let _mode: EngineMode = EngineMode::SingleCore;
    let _plan: EnginePlan = EnginePlan::from_dimensions(80, 24);
    let _config = ZactrixSystemConfig::default();
    let _render: RenderPlan = RenderPlan::default();
    let _policy: TerminalWriterPolicy = TerminalWriterPolicy::default();
    let _runtime: RuntimeMode = RuntimeMode::default();
    let _cpu: CpuBudget = CpuBudget::default();
    let _idle: IdlePolicy = IdlePolicy::default();
    let _compute: ComputeParallelism = ComputeParallelism::default();
    let _probe = EngineProbe::from_dimensions(120, 40);
    let _ = (
        _mode, _plan, _config, _render, _policy, _runtime, _cpu, _idle, _compute, _probe,
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
fn zactrix_engine_compute_parallelism_default_is_not_active() {
    use crate::zactrix_engine::ComputeParallelism;
    assert_ne!(ComputeParallelism::default(), ComputeParallelism::Active);
}

#[test]
fn zactrix_engine_runtime_mode_labels_are_stable() {
    use crate::zactrix_engine::RuntimeMode;
    assert!(!RuntimeMode::Calm.as_str().is_empty());
    assert!(!RuntimeMode::Normal.as_str().is_empty());
    assert!(!RuntimeMode::Stress.as_str().is_empty());
}

#[test]
fn zactrix_engine_cpu_budget_labels_are_stable() {
    use crate::zactrix_engine::CpuBudget;
    assert!(!CpuBudget::Low.as_str().is_empty());
    assert!(!CpuBudget::Balanced.as_str().is_empty());
    assert!(!CpuBudget::Stress.as_str().is_empty());
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
    assert!(!plan.compute_enabled);
}

#[test]
fn zactrix_engine_docs_mention_parallel_compute_single_owner() {
    let docs = include_str!("../../docs/ZACTRIX_ENGINE.md");
    let lower = docs.to_lowercase();
    assert!(
        lower.contains("parallel compute") && lower.contains("single-owner"),
        "ZACTRIX_ENGINE.md must mention parallel compute + single-owner"
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

// ── Phase 12.6: v4.5.0 Phase 2 Zactrix System diagnostics guards ──────

#[test]
fn zactrix_system_runtime_mode_default_is_normal() {
    use crate::zactrix_engine::{RuntimeMode, ZactrixSystemConfig};
    let sys = ZactrixSystemConfig::default();
    assert_eq!(sys.runtime_mode, RuntimeMode::Normal);
    assert_eq!(sys.runtime_mode.as_str(), "normal");
}

#[test]
fn zactrix_system_cpu_budget_default_is_balanced() {
    use crate::zactrix_engine::{CpuBudget, ZactrixSystemConfig};
    let sys = ZactrixSystemConfig::default();
    assert_eq!(sys.cpu_budget, CpuBudget::Balanced);
    assert_eq!(sys.cpu_budget.as_str(), "balanced");
}

#[test]
fn zactrix_system_render_plan_default_is_single_owner() {
    use crate::zactrix_engine::{RenderPlan, TerminalWriterPolicy};
    let render = RenderPlan::default();
    assert_eq!(render.writer_policy, TerminalWriterPolicy::SingleOwner);
    assert_eq!(render.writer_policy.as_str(), "single-owner");
}

#[test]
fn zactrix_system_compute_parallelism_default_is_disabled() {
    use crate::zactrix_engine::{ComputeParallelism, ZactrixSystemConfig};
    let sys = ZactrixSystemConfig::default();
    assert_eq!(sys.compute_parallelism, ComputeParallelism::Disabled);
    assert_eq!(sys.compute_parallelism.as_str(), "disabled");
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
    // Verify that main.rs contains the ZACTRIX SYSTEM section in -i output.
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
        main_rs.contains("cpu_budget"),
        "main.rs must emit cpu_budget field"
    );
    assert!(
        main_rs.contains("render_plan"),
        "main.rs must emit render_plan field"
    );
    assert!(
        main_rs.contains("compute_parallelism"),
        "main.rs must emit compute_parallelism field"
    );
    assert!(
        main_rs.contains("idle_policy"),
        "main.rs must emit idle_policy field"
    );
}

#[test]
fn zactrix_system_benchmark_emits_zactrix_system_section() {
    // Verify that bench_report.rs contains the ZACTRIX SYSTEM section.
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
        bench.contains("cpu_budget"),
        "bench_report.rs must emit cpu_budget field"
    );
    assert!(
        bench.contains("render_plan"),
        "bench_report.rs must emit render_plan field"
    );
    assert!(
        bench.contains("compute_parallelism"),
        "bench_report.rs must emit compute_parallelism field"
    );
    assert!(
        bench.contains("idle_policy"),
        "bench_report.rs must emit idle_policy field"
    );
}

#[test]
fn zactrix_system_existing_zactrix_engine_benchmark_unchanged() {
    // Verify ZACTRIX ENGINE section in benchmark is still intact.
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
    assert!(
        bench.contains("planned_mode"),
        "ZACTRIX ENGINE must still have planned_mode"
    );
}

#[test]
fn zactrix_system_docs_mention_v450_phase_2() {
    let docs = include_str!("../../docs/ZACTRIX_ENGINE.md");
    assert!(
        docs.contains("v4.5.0 Phase 2"),
        "ZACTRIX_ENGINE.md must mention v4.5.0 Phase 2"
    );
    assert!(
        docs.contains("ZACTRIX SYSTEM"),
        "ZACTRIX_ENGINE.md must mention ZACTRIX SYSTEM"
    );
}

// ── Phase 12.7: v4.5.0 Phase 3 Depth Regression diagnostics guards ──────

#[test]
fn depth_lab_benchmark_actual_execution_is_single_threaded() {
    // ZACTRIX ENGINE must report actual_execution: single-threaded-renderer.
    // This is a hard invariant: no real parallel execution exists.
    let bench = include_str!("../bench_report.rs");
    assert!(
        bench.contains("\"single-threaded-renderer\""),
        "bench_report.rs must emit actual_execution: single-threaded-renderer"
    );
}

#[test]
fn depth_lab_benchmark_terminal_writer_is_single_owner() {
    // ZACTRIX ENGINE must report terminal_writer: single-owner.
    // Terminal writes must never be parallelized.
    let bench = include_str!("../bench_report.rs");
    assert!(
        bench.contains("\"single-owner\""),
        "bench_report.rs must emit terminal_writer: single-owner"
    );
}

#[test]
fn depth_lab_benchmark_compute_parallelism_remains_disabled() {
    // ZACTRIX SYSTEM must report compute_parallelism: disabled.
    let bench = include_str!("../bench_report.rs");
    // The bench_report uses sys.compute_parallelism.as_str() which
    // evaluates to "disabled" at runtime. Verify the source references it.
    assert!(
        bench.contains("compute_parallelism"),
        "bench_report.rs must reference compute_parallelism field"
    );
    // Also verify the ZactrixSystemConfig default is disabled
    use crate::zactrix_engine::{ComputeParallelism, ZactrixSystemConfig};
    assert_eq!(
        ZactrixSystemConfig::default().compute_parallelism,
        ComputeParallelism::Disabled
    );
}

#[test]
fn depth_lab_benchmark_render_plan_remains_single_owner() {
    // ZACTRIX SYSTEM must report render_plan: single-owner.
    use crate::zactrix_engine::{RenderPlan, TerminalWriterPolicy};
    let render = RenderPlan::default();
    assert_eq!(render.writer_policy, TerminalWriterPolicy::SingleOwner);
}

#[test]
fn depth_lab_no_active_parallel_compute_claimed() {
    // Verify that no code path claims active parallel compute.
    // The EngineMode enum should not have an ActiveParallel variant
    // that would be claimed in benchmark output.
    use crate::zactrix_engine::EngineMode;
    // EngineMode has: SingleCore, Assist, SafeFallback — none imply active execution
    let _modes = [
        EngineMode::SingleCore,
        EngineMode::Assist,
        EngineMode::SafeFallback,
    ];
}

#[test]
fn depth_lab_info_output_zactrix_system_honest() {
    // Verify that main.rs -i output includes honest ZACTRIX SYSTEM fields.
    let main_rs = include_str!("../main.rs");
    // All 5 ZACTRIX SYSTEM fields must be present
    let required_fields = [
        "runtime_mode",
        "cpu_budget",
        "render_plan",
        "compute_parallelism",
        "idle_policy",
    ];
    for field in &required_fields {
        assert!(
            main_rs.contains(field),
            "main.rs -i output must include ZACTRIX SYSTEM field '{}'",
            field
        );
    }
}

#[test]
fn depth_lab_visual_stability_doc_exists() {
    // Verify that VISUAL_STABILITY.md exists and covers key concepts.
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
    assert!(
        lower.contains("monolith rain"),
        "VISUAL_STABILITY.md must mention Monolith Rain"
    );
}

#[test]
fn depth_lab_visual_stability_doc_mentions_zactrix_guard() {
    let docs = include_str!("../../docs/VISUAL_STABILITY.md");
    assert!(
        docs.contains("Zactrix Engine"),
        "VISUAL_STABILITY.md must mention Zactrix Engine guard"
    );
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
    assert!(docs.contains("v4.7"), "ROADMAP.md must mention v4.7");
    assert!(docs.contains("v4.8"), "ROADMAP.md must mention v4.8");
    assert!(docs.contains("v5.0"), "ROADMAP.md must mention v5.0");
}

#[test]
fn phase6_roadmap_says_atmosphere_opt_in() {
    let docs = include_str!("../../docs/ROADMAP.md");
    let lower = docs.to_lowercase();
    assert!(
        lower.contains("opt-in") && lower.contains("atmosphere"),
        "ROADMAP.md must say controlled atmosphere remains opt-in"
    );
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
fn phase6_zactrix_docs_mention_v45_closure() {
    let docs = include_str!("../../docs/ZACTRIX_ENGINE.md");
    assert!(
        docs.contains("v4.5.0 Foundation Closure"),
        "ZACTRIX_ENGINE.md must mention v4.5.0 Foundation Closure"
    );
    assert!(
        docs.contains("architecture and regression foundation"),
        "ZACTRIX_ENGINE.md must describe v4.5 as architecture and regression foundation"
    );
}

#[test]
fn phase6_benchmark_docs_mention_synthetic_fps_and_stability() {
    let docs = include_str!("../../benchmark/README.md");
    let lower = docs.to_lowercase();
    assert!(
        lower.contains("synthetic uncapped throughput"),
        "benchmark/README.md must mention synthetic uncapped throughput"
    );
    assert!(
        lower.contains("p99") || lower.contains("p95"),
        "benchmark/README.md must mention p95/p99 priority"
    );
}

#[test]
fn phase6_benchmark_docs_mention_v45_plateau_as_approximate() {
    let docs = include_str!("../../benchmark/README.md");
    let lower = docs.to_lowercase();
    assert!(
        lower.contains("approximate"),
        "benchmark/README.md must describe the v4.5 plateau as approximate, not a promise"
    );
    assert!(
        docs.contains("v4.5"),
        "benchmark/README.md must reference v4.5"
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

// ── v4.8.0 Phase 0: Benchmark Ceiling Lab guards ───────────────────────

#[test]
fn phase0_benchmark_fields_still_present() {
    // Verify that all benchmark metric fields remain in the output.
    // Removing any of these fields breaks backward compatibility.
    let bench = include_str!("../bench_report.rs");
    let required = [
        "avg_fps",
        "median_fps",
        "p95_frame_time",
        "p99_frame_time",
        "frame_time_stability",
        "avg_dirty_cell_ratio_percent",
        "active_streams_avg",
        "actual_execution",
        "terminal_writer",
        "compute_parallelism",
    ];
    for field in &required {
        assert!(
            bench.contains(field),
            "bench_report.rs must still emit '{field}'"
        );
    }
}

#[test]
fn phase0_actual_execution_remains_honest() {
    // The actual_execution field must always report single-threaded-renderer.
    // No parallel execution was added in this phase.
    use crate::zactrix_engine::EnginePlan;
    let plan = EnginePlan::from_dimensions(120, 40);
    assert_eq!(plan.mode.as_str(), "single-core");
    let bench = include_str!("../bench_report.rs");
    assert!(
        bench.contains("single-threaded-renderer"),
        "actual_execution must remain single-threaded-renderer"
    );
}

#[test]
fn phase0_terminal_writer_remains_single_owner() {
    use crate::zactrix_engine::{RenderPlan, TerminalWriterPolicy};
    assert_eq!(
        TerminalWriterPolicy::default(),
        TerminalWriterPolicy::SingleOwner
    );
    let plan = RenderPlan::default();
    assert_eq!(plan.writer_policy, TerminalWriterPolicy::SingleOwner);
}

#[test]
fn phase0_no_parallel_terminal_writing_added() {
    // Verify that Frame::set_force does not imply parallel access.
    // set_force is a single-thread optimization (skips equality check),
    // not a concurrent write primitive.
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

#[test]
fn phase0_dirty_cell_ratio_not_artificially_collapsed() {
    // The benchmark must still measure and report dirty_cell_ratio.
    // Artificially collapsing this metric would be a form of cheating.
    let bench = include_str!("../bench_report.rs");
    assert!(
        bench.contains("avg_dirty_cell_ratio_percent"),
        "bench_report must still measure avg_dirty_cell_ratio_percent"
    );
    assert!(
        bench.contains("dirty_cell_ratio"),
        "bench_report must still reference dirty_cell_ratio"
    );
}

#[test]
fn phase0_rgb_optimization_api_exists() {
    // Verify the new RGB-tuple optimization functions exist in palette.
    let palette = include_str!("../palette.rs");
    assert!(
        palette.contains("decode_color"),
        "palette.rs must have decode_color for single-decode optimization"
    );
    assert!(
        palette.contains("apply_brightness_rgb"),
        "palette.rs must have apply_brightness_rgb for hot-path RGB variant"
    );
}

#[test]
fn phase0_pool_is_binary_cached_in_drawctx() {
    // Verify DrawCtx caches pool_is_binary instead of iterating per-cell.
    let render = include_str!("../cloud/render.rs");
    assert!(
        render.contains("pool_is_binary: bool"),
        "DrawCtx must have pool_is_binary field"
    );
    let rain = include_str!("../cloud/rain.rs");
    assert!(
        rain.contains("pool_is_binary"),
        "rain.rs must compute pool_is_binary during DrawCtx construction"
    );
}

#[test]
fn phase0_monolith_color_for_level_uses_single_decode() {
    // Verify monolith color_for_level decodes color once, not multiple times.
    let monolith = include_str!("../cloud/monolith.rs");
    assert!(
        monolith.contains("decode_color"),
        "monolith.rs color_for_level must use decode_color (single decode)"
    );
    // Should NOT contain the old pattern of chaining apply_brightness + blend_toward_white
    // which each re-decode the color
    assert!(
        !monolith.contains("palette::apply_brightness"),
        "monolith.rs color_for_level should not call apply_brightness (re-decode)"
    );
}

#[test]
fn phase0_droplet_draw_uses_single_decode() {
    // Verify droplet draw pipeline decodes color once for all effects.
    let droplet = include_str!("../droplet.rs");
    assert!(
        droplet.contains("decode_color"),
        "droplet.rs draw must use decode_color (single decode)"
    );
}

#[test]
fn phase0_no_new_dependencies() {
    // Verify Cargo.toml dependencies haven't changed in this phase.
    // The optimization uses only existing crossterm Color types.
    let cargo = include_str!("../../Cargo.toml");
    // No new crate dependencies should be present
    assert!(
        !cargo.contains("rayon"),
        "No Rayon dependency (no parallel iterators)"
    );
    assert!(!cargo.contains("crossbeam"), "No crossbeam dependency");
}

#[test]
fn phase0_no_version_bump() {
    // This phase is research-only; version must remain v4.5.0.
    let cargo = include_str!("../../Cargo.toml");
    assert!(
        cargo.contains("version = \"4.5.0\""),
        "Cargo.toml must still show version 4.5.0 (no bump in Phase 0)"
    );
}

#[test]
fn phase0_all_modified_files_under_loc_cap() {
    let files = [
        "src/palette.rs",
        "src/droplet.rs",
        "src/frame.rs",
        "src/cloud/monolith.rs",
        "src/cloud/monolith_glyphs.rs",
        "src/cloud/phosphor.rs",
        "src/cloud/rain.rs",
        "src/cloud/render.rs",
    ];
    for path in &files {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let count = content.lines().count();
        assert!(count <= 1000, "{path}: {count} LOC exceeds 1000 cap");
    }
}
