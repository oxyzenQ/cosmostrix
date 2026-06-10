// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Zactrix Engine/Cache/Core doc guards, planner tests, and architecture split guards.

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
