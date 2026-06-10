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
    files.push(std::path::PathBuf::from("CHANGELOG.md"));

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

// ── Phase 9.1: README / Changelog guard tests ──────────────────────────

#[test]
fn readme_must_not_contain_release_notes_section() {
    let readme = include_str!("../README.md");
    let lowercase = readme.to_lowercase();
    assert!(
        !lowercase.contains("\n## release notes"),
        "README must not contain a top-level 'Release notes' section"
    );
    assert!(
        !lowercase.contains("\n### release notes"),
        "README must not contain a second-level 'Release notes' section"
    );
}

#[test]
fn readme_must_not_contain_old_version_history_blocks() {
    let readme = include_str!("../README.md");
    // These old version-history headings must not appear in the landing page.
    assert!(
        !readme.contains("v3.1.0 (in development)"),
        "README must not contain stale 'v3.1.0 (in development)'"
    );
    assert!(
        !readme.contains("\n### v2.2.0"),
        "README must not contain v2.2.0 release note heading"
    );
    assert!(
        !readme.contains("\n### v2.1.0"),
        "README must not contain v2.1.0 release note heading"
    );
    assert!(
        !readme.contains("\n### v2.0.0"),
        "README must not contain v2.0.0 release note heading"
    );
}

#[test]
fn readme_must_link_to_changelog() {
    let readme = include_str!("../README.md");
    assert!(
        readme.contains("CHANGELOG.md"),
        "README must link to CHANGELOG.md"
    );
}

#[test]
fn changelog_exists_and_contains_historical_notes() {
    let changelog = include_str!("../CHANGELOG.md");
    assert!(
        changelog.contains("## v3.1.0"),
        "CHANGELOG must contain v3.1.0 entry"
    );
    assert!(
        changelog.contains("## v2.2.0"),
        "CHANGELOG must contain v2.2.0 entry"
    );
    assert!(
        changelog.contains("## v2.1.0"),
        "CHANGELOG must contain v2.1.0 entry"
    );
    assert!(
        changelog.contains("## v2.0.0"),
        "CHANGELOG must contain v2.0.0 entry"
    );
    assert!(
        !changelog.contains("in development)"),
        "CHANGELOG must not contain stale 'in development' wording"
    );
}

#[test]
fn readme_keeps_canonical_repo_casing() {
    let readme = include_str!("../README.md");
    assert!(
        readme.contains("github.com/oxyzenQ"),
        "README must contain canonical repo casing github.com/oxyzenQ"
    );
    let lower = "github.com/".to_string() + concat!("oxyzen", "q");
    assert!(
        !readme.contains(&lower),
        "README must not contain wrong-cased repo owner"
    );
}

#[test]
fn changelog_keeps_canonical_repo_casing() {
    let changelog = include_str!("../CHANGELOG.md");
    let lower = "github.com/".to_string() + concat!("oxyzen", "q");
    assert!(
        !changelog.contains(&lower),
        "CHANGELOG must not contain wrong-cased repo owner"
    );
}

#[test]
fn readme_stays_under_350_lines() {
    let readme = include_str!("../README.md");
    let count = readme.lines().count();
    assert!(
        count <= 350,
        "README has {count} lines, must stay under 350"
    );
}

// ── Phase 10.6: description/tagline consistency guard tests ────────────

#[test]
fn cargo_toml_uses_canonical_tagline() {
    let cargo = include_str!("../Cargo.toml");
    assert!(
        cargo.contains("description = \"Production-grade cinematic Matrix rain renderer for serious terminal environments.\""),
        "Cargo.toml description must use the canonical tagline"
    );
}

#[test]
fn readme_uses_canonical_tagline() {
    let readme = include_str!("../README.md");
    assert!(
        readme.contains(
            "Production-grade cinematic Matrix rain renderer for serious terminal environments."
        ),
        "README.md must contain the canonical tagline"
    );
}

#[test]
fn runtime_identity_uses_canonical_tagline() {
    let ri = include_str!("renderer_info.rs");
    assert!(
        ri.contains(
            "production-grade cinematic Matrix rain renderer for serious terminal environments."
        ),
        "renderer_info.rs identity must use the canonical tagline"
    );
}

#[test]
fn readme_does_not_contain_stale_high_performance_tagline() {
    let readme = include_str!("../README.md");
    assert!(
        !readme.contains("High-performance cinematic Matrix rain renderer for the terminal."),
        "README must not contain the old 'High-performance' tagline"
    );
}

#[test]
fn changelog_uses_568_not_570() {
    let changelog = include_str!("../CHANGELOG.md");
    assert!(
        changelog.contains("568 deterministic tests"),
        "CHANGELOG.md must say 568 deterministic tests"
    );
    assert!(
        !changelog.contains("570 deterministic tests"),
        "CHANGELOG.md must not contain stale 570 deterministic tests"
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

// ── Phase 11: Release Candidate Hardening guard tests ────────────────────

#[test]
fn release_candidate_doc_exists_and_covers_checklist() {
    let docs = include_str!("../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("cargo clippy"),
        "RELEASE_CANDIDATE.md should mention cargo clippy"
    );
    assert!(
        docs.contains("cargo test"),
        "RELEASE_CANDIDATE.md should mention cargo test"
    );
}

#[test]
fn release_candidate_doc_mentions_no_version_bump_until_release() {
    let docs = include_str!("../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("Do not bump the version") || docs.contains("do not bump the version"),
        "RELEASE_CANDIDATE.md should warn against premature version bumps"
    );
}

#[test]
fn release_candidate_doc_includes_runtime_smoke_commands() {
    let docs = include_str!("../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("\"$BIN\" -i") || docs.contains("$BIN -i"),
        "RELEASE_CANDIDATE.md should include -i runtime smoke command"
    );
    assert!(
        docs.contains("\"$BIN\" --benchmark") || docs.contains("$BIN --benchmark"),
        "RELEASE_CANDIDATE.md should include --benchmark runtime smoke command"
    );
}

#[test]
fn release_candidate_doc_includes_controlled_live_config_smoke() {
    let docs = include_str!("../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("controlled-live"),
        "RELEASE_CANDIDATE.md should mention controlled-live config smoke"
    );
}

#[test]
fn release_candidate_doc_includes_readme_changelog_guard() {
    let docs = include_str!("../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("CHANGELOG") && docs.contains("README"),
        "RELEASE_CANDIDATE.md should mention both CHANGELOG and README guards"
    );
}

#[test]
fn benchmark_docs_mention_fps_is_synthetic_uncapped() {
    let docs = include_str!("../benchmark/README.md");
    assert!(
        docs.to_lowercase().contains("synthetic") && docs.to_lowercase().contains("uncapped"),
        "benchmark/README.md should state FPS is synthetic/uncapped"
    );
}

#[test]
fn benchmark_docs_mention_stability_more_important_than_peak_fps() {
    let docs = include_str!("../benchmark/README.md");
    assert!(
        docs.contains("p99")
            && (docs.to_lowercase().contains("stability")
                || docs.to_lowercase().contains("more than")),
        "benchmark/README.md should emphasize stability over peak FPS"
    );
}

// ── Phase 11.7: README GIF-first demo / old asset cleanup ────────────────

#[test]
fn readme_references_v4_demo_gif() {
    let readme = include_str!("../README.md");
    assert!(
        readme.contains("assets/cosmostrix-v4-demo.gif"),
        "README must reference the v4 demo GIF"
    );
}

#[test]
fn readme_references_v4_demo_video() {
    let readme = include_str!("../README.md");
    assert!(
        readme.contains("assets/cosmostrix-v4-demo.mp4"),
        "README must reference the v4 demo video"
    );
}

#[test]
fn readme_references_v4_demo_binary_poster() {
    let readme = include_str!("../README.md");
    assert!(
        readme.contains("assets/cosmostrix-v4-demo-binary.png"),
        "README must reference the v4 binary demo poster"
    );
}

#[test]
fn readme_references_v4_demo_retro_poster() {
    let readme = include_str!("../README.md");
    assert!(
        readme.contains("assets/cosmostrix-v4-demo-retro.png"),
        "README must reference the v4 retro demo poster"
    );
}

#[test]
fn v4_demo_gif_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v4-demo.gif");
    assert!(path.exists(), "assets/cosmostrix-v4-demo.gif must exist");
}

#[test]
fn v4_demo_video_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v4-demo.mp4");
    assert!(path.exists(), "assets/cosmostrix-v4-demo.mp4 must exist");
}

#[test]
fn v4_demo_binary_poster_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v4-demo-binary.png");
    assert!(
        path.exists(),
        "assets/cosmostrix-v4-demo-binary.png must exist"
    );
}

#[test]
fn v4_demo_retro_poster_asset_exists() {
    let path = std::path::Path::new("assets/cosmostrix-v4-demo-retro.png");
    assert!(
        path.exists(),
        "assets/cosmostrix-v4-demo-retro.png must exist"
    );
}

#[test]
fn readme_gif_appears_before_poster_pngs() {
    let readme = include_str!("../README.md");
    let gif_pos = readme
        .find("cosmostrix-v4-demo.gif")
        .expect("README must contain GIF ref");
    let binary_pos = readme
        .find("cosmostrix-v4-demo-binary.png")
        .expect("README must contain binary PNG ref");
    let retro_pos = readme
        .find("cosmostrix-v4-demo-retro.png")
        .expect("README must contain retro PNG ref");
    assert!(
        gif_pos < binary_pos,
        "README GIF reference must appear before binary PNG"
    );
    assert!(
        binary_pos < retro_pos,
        "README binary PNG must appear before retro PNG"
    );
}

#[test]
fn readme_does_not_use_old_demo_gif_as_primary() {
    let readme = include_str!("../README.md");
    assert!(
        !readme.contains("cosmostrix-demo.gif"),
        "README must not reference the old demo GIF"
    );
}

#[test]
fn old_demo_gif_removed_from_assets() {
    let path = std::path::Path::new("assets/cosmostrix-demo.gif");
    assert!(
        !path.exists(),
        "Old assets/cosmostrix-demo.gif should have been removed"
    );
}

// ── Phase 12.1: v4.0.0 release metadata guard tests ──────────────────────

#[test]
fn changelog_has_v400_entry_above_v390() {
    let changelog = include_str!("../CHANGELOG.md");
    let v400_pos = changelog
        .find("## v4.0.0")
        .expect("CHANGELOG must contain v4.0.0 entry");
    let v390_pos = changelog
        .find("## v3.9.0")
        .expect("CHANGELOG must contain v3.9.0 entry");
    assert!(
        v400_pos < v390_pos,
        "CHANGELOG v4.0.0 entry must appear above v3.9.0"
    );
}

#[test]
fn changelog_v400_mentions_default_runtime_protected_identity() {
    let changelog = include_str!("../CHANGELOG.md");
    let lower = changelog.to_lowercase();
    assert!(
        lower.contains("application_mode = disabled")
            && lower.contains("effective_runtime = identity")
            && lower.contains("shadow_risk = identity"),
        "CHANGELOG v4.0.0 must mention default runtime remains protected/identity"
    );
}

#[test]
fn changelog_v400_mentions_no_multithreaded_terminal_rendering() {
    let changelog = include_str!("../CHANGELOG.md");
    let lower = changelog.to_lowercase();
    assert!(
        lower.contains("no actual multithreaded terminal rendering")
            || lower.contains("single-owner"),
        "CHANGELOG v4.0.0 must mention no multithreaded terminal rendering"
    );
}

#[test]
fn changelog_v400_mentions_demo_refresh() {
    let changelog = include_str!("../CHANGELOG.md");
    assert!(
        changelog.to_lowercase().contains("demo refresh")
            || changelog.to_lowercase().contains("gif-first"),
        "CHANGELOG v4.0.0 must mention demo refresh"
    );
}

#[test]
fn cargo_toml_version_matches_changelog_latest() {
    let cargo = include_str!("../Cargo.toml");
    assert!(
        cargo.contains("version = \"4.0.1\""),
        "Cargo.toml must have version = \"4.0.1\""
    );
    assert!(
        !cargo.contains("version = \"3.9.0\""),
        "Cargo.toml must not contain old version 3.9.0"
    );
}

#[test]
fn readme_uses_v401_tag_in_install_example() {
    let readme = include_str!("../README.md");
    assert!(
        readme.contains("TAG=\"v4.0.1\""),
        "README install example must use TAG=\"v4.0.1\" as the current release tag"
    );
}

// ── Phase 12.2: v4.0.1 release metadata guard tests ──────────────────────

#[test]
fn aur_pkgbuild_pkgver_matches_release() {
    let pkgbuild = include_str!("../aur/cosmostrix-bin/PKGBUILD");
    assert!(
        pkgbuild.contains("pkgver=4.0.1"),
        "PKGBUILD must have pkgver=4.0.1"
    );
    assert!(
        !pkgbuild.contains("pkgver=4.0.0"),
        "PKGBUILD must not contain old pkgver=4.0.0"
    );
}

#[test]
fn aur_srcinfo_pkgver_matches_release() {
    let srcinfo = include_str!("../aur/cosmostrix-bin/.SRCINFO");
    assert!(
        srcinfo.contains("pkgver = 4.0.1"),
        "SRCINFO must have pkgver = 4.0.1"
    );
    assert!(
        !srcinfo.contains("pkgver = 4.0.0"),
        "SRCINFO must not contain old pkgver = 4.0.0"
    );
}

#[test]
fn no_active_metadata_still_uses_v400() {
    // Active metadata (Cargo.toml, PKGBUILD, .SRCINFO, README install tag)
    // must not reference 4.0.0. Historical CHANGELOG references are allowed.
    let cargo = include_str!("../Cargo.toml");
    let pkgbuild = include_str!("../aur/cosmostrix-bin/PKGBUILD");
    let srcinfo = include_str!("../aur/cosmostrix-bin/.SRCINFO");

    // Cargo.toml version must not be 4.0.0
    assert!(
        !cargo.contains("version = \"4.0.0\""),
        "Cargo.toml must not have version = \"4.0.0\""
    );
    // PKGBUILD pkgver must not be 4.0.0
    assert!(
        !pkgbuild.contains("pkgver=4.0.0"),
        "PKGBUILD must not have pkgver=4.0.0"
    );
    // .SRCINFO pkgver must not be 4.0.0
    assert!(
        !srcinfo.contains("pkgver = 4.0.0"),
        "SRCINFO must not have pkgver = 4.0.0"
    );
}

// ── Phase 12.3: release workflow authentication guard tests ──────────────

#[test]
fn release_workflow_has_contents_write_permission() {
    let workflow = include_str!("../.github/workflows/release.yml");
    assert!(
        workflow.contains("contents: write"),
        "release workflow must grant contents: write permission"
    );
}

#[test]
fn release_workflow_passes_github_token_to_release_action() {
    let workflow = include_str!("../.github/workflows/release.yml");
    assert!(
        workflow.contains("GITHUB_TOKEN") && workflow.contains("secrets.GITHUB_TOKEN"),
        "release workflow must pass GITHUB_TOKEN to the release action"
    );
}

#[test]
fn release_workflow_publish_job_has_permissions() {
    let workflow = include_str!("../.github/workflows/release.yml");
    // The publish_release job must have its own permissions block with
    // contents: write, not rely solely on top-level inheritance.
    let publish_marker = "publish_release:";
    let publish_pos = workflow
        .find(publish_marker)
        .expect("release workflow must contain publish_release job");
    let perm_pos = workflow[publish_pos..]
        .find("permissions:")
        .expect("publish_release job must have a permissions block");
    let perm_section = &workflow[publish_pos + perm_pos..];
    assert!(
        perm_section.contains("contents: write"),
        "publish_release job permissions must include contents: write"
    );
}

#[test]
fn release_candidate_doc_mentions_auth_requirement() {
    let docs = include_str!("../docs/RELEASE_CANDIDATE.md");
    assert!(
        docs.contains("contents: write")
            && (docs.contains("GITHUB_TOKEN") || docs.contains("authentication")),
        "RELEASE_CANDIDATE.md must document the release workflow authentication requirement"
    );
}
