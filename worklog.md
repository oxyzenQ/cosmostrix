---
Task ID: 1
Agent: main
Task: v3.3.0 Engine Architecture Split + Core Optimization Audit

Work Log:
- Verified mod.rs split already complete (711 LOC, well under 800 target): scene_runtime.rs (143 LOC) and runtime_controls.rs (168 LOC) extracted in v3.2.1 session
- Task B: Relieved test pressure — extracted 306 lines of viewport edge fade + phosphor edge cap tests from tests/mod.rs (919→613 LOC) into new tests_edge_fade.rs (316 LOC)
- Task C: Core optimization audit — identified 10 safe optimization opportunities across 9 hot-path files; applied 3 zero-risk changes:
  1. rain.rs: Removed dead `empty` binding (dead code elimination)
  2. droplet.rs: Hoisted `blank_cell(bg)` out of tail cleanup loop (eliminates per-tail-cell struct construction)
  3. phosphor.rs: Fixed redundant cell read in GlyphCorruption anomaly path (cell_at_index_ref + cell_at_index → single ref then conditional copy)
- Reverted spawn.rs col_stat caching (conflicts with Rust borrow checker due to interleaved mutable borrows)
- Task D: Created docs/SIMD_FEASIBILITY.md (267-line audit) — conclusion: manual SIMD not recommended; x86-64-v3 LLVM auto-vectorization sufficient
- Task E: Updated README.md — added SIMD doc link and architecture split note to v2.2.0 section
- All 211 tests pass, clippy clean, fmt clean, build.sh check-all passes
- Pro-linux-v3 build successful, benchmarks run, -i and --help-detail verified
- No version bump (stays v3.2.0), no tag, no release

Stage Summary:
- tests/mod.rs: 919 → 613 LOC (306 lines extracted to tests_edge_fade.rs)
- cloud/mod.rs: already 711 LOC (extraction from previous session)
- 3 safe micro-optimizations applied (rain.rs dead code, droplet.rs blank hoist, phosphor.rs redundant read fix)
- 7 optimization opportunities documented but not applied (require unsafe or risk visual changes)
- SIMD feasibility documented: NOT recommended (5-15% estimated impact, high maintenance cost)
- All files under 1000 LOC cap maintained
- Commit pending with all changes
