<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# SIMD Feasibility Audit: Manual Vectorization for Cosmostrix Rain Renderer

> **Document ID**: SIMD-FEASIBILITY-001
> **Date**: 2026-06
> **Scope**: x86-64 SSE/AVX/AVX2 manual SIMD analysis
> **Status**: **NOT RECOMMENDED** — see Recommendation (§6)

---

## 1. Executive Summary

This document evaluates whether manual SIMD intrinsics (x86-64 SSE/AVX/AVX2)
would yield meaningful performance improvements for the Cosmostrix terminal rain
renderer.

**Conclusion: manual SIMD is NOT recommended at this time.**

The renderer already achieves stable 60 fps on typical terminals (200×50 =
10,000 cells) using LLVM's auto-vectorization with the x86-64-v3 target
baseline (AVX2 + SSE4.2 + BMI1/BMI2/FMA). The per-cell workload is dominated
by branching logic, indirect memory access, and enum dispatch — all of which
defeat SIMD vectorization. Introducing `unsafe` SIMD intrinsics would violate
the project's no-new-unsafe renderer/core policy, add significant maintenance
burden, and yield an estimated 5–15% improvement that is imperceptible at
already-adequate frame rates.

For SIMD to become worthwhile, the workload would need to scale by ~800×
(8M+ cells/frame), data layout would need restructuring to contiguous SoA format,
and per-cell conditional branches would need elimination.

---

## 2. Current Architecture & Toolchain

### 2.1 Target Baseline

The `pro-linux-v3` cargo alias (`.cargo/config.toml`) passes
`-C target-cpu=x86-64-v3`, enabling AVX, AVX2, FMA, BMI1/BMI2 at the ISA
level. LLVM's auto-vectorizer is active with the most aggressive settings:

- `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`
- `overflow-checks = false` (no runtime integer overflow)

### 2.2 Core Data Structures

- **`cell.rs::Cell`** (~24 bytes, `Copy`): `ch: char`, `fg: Option<Color>`,
  `bg: Option<Color>`, `bold: bool` — heterogeneous enum payloads prevent
  SIMD packing.
- **`frame.rs::Frame`**: `Vec<Cell>` (row-major), `Vec<u32>` (generation),
  `BitVec` (dirty map), `SmallVec<[usize; 64]>` (dirty indices).
- **Phosphor arrays** (column-major `Vec`): `Vec<u8>` (energy), `Vec<Option<Color>>`
  (base_fg), `Vec<char>` (base_ch), `BitVec` (fresh flags).

### 2.3 Constraints (per `RULES.md`)

Rust 1.81.0 stable, Clippy `-D warnings`, no new unsafe in renderer/core paths
unless explicitly audited, source files under 1,000 lines.

---

## 3. Hot-Path Analysis

### 3.1 Per-Frame Cost Profile (200×50 = 10,000 cells)

| Subsystem | Function | Calls/Frame (est.) | Bottleneck |
|-----------|----------|-------------------|-------------|
| Frame mutation | `frame.rs::set()` | 15K–25K | Bounds check + branches |
| Phosphor decay | `cloud/phosphor.rs::phosphor_decay_pass()` | 3×10K | Conditional cascade |
| Droplet draw | `droplet.rs::Droplet::draw()` | Per-droplet×cells | Palette blending |
| Monolith draw | `cloud/monolith.rs::draw_segments()` | Per-stream×cells | Color lookup + branch |
| Color blending | `palette.rs::apply_brightness()` | 3–5×/cell | `color_to_rgb()` dispatch |
| Color blending | `palette.rs::blend_toward_white()` | 2–4×/cell | Same dispatch |
| Color decode | `palette.rs::color_to_rgb()` | 5–8×/cell | 7-way enum match |
| Edge fade | `droplet.rs::viewport_edge_fade()` | Per-drawn cell | Branch per edge zone |
| Atmospheric | `phosphor.rs::apply_atmospheric_frame_effects()` | 1×10K | Color mode branches |
| Spawn | `cloud/spawn.rs::spawn_droplets()` | O(droplets) | RNG sampling |

### 3.2 Aggregate Load

Total f32 operations per frame: ~150K–240K multiply-adds (5–8 color calls ×
3 channels × 10K cells). This is ~0.6–1.0 MFLOP/s at 60 fps — trivially small
for any modern core (~16 GFLOP/s scalar, ~128 GFLOP/s AVX2 f32). **The
bottleneck is dispatch overhead, branch misprediction, and cache access — not
arithmetic throughput.**

---

## 4. SIMD Opportunity Assessment

### 4.1 Why SIMD Does Not Help

**A. Per-cell branching defeats lane parallelism.** SIMD processes N elements
in lockstep. When cells take different paths (e.g., `Color::Rgb` vs
`Color::AnsiValue`, head vs tail vs middle), lanes are wasted on unused
results or serialized to scalar fallback. Nearly every per-cell operation in
Cosmostrix has conditional logic:

```rust
// palette.rs:217 — early-return + enum dispatch
if factor <= 0.0 || matches!(color, Color::Reset) { return color; }
let (r, g, b) = color_to_rgb(color); // 7-way match on Color

// frame.rs:185 — 5-way conditional chain per call
if let Some(i) = self.index(x, y) {
    let cur = if self.cell_gen[...] == self.gen { ... } else { ... };
    if cur == cell { return; }
    ...
}

// phosphor.rs:176-314 — ~8 conditional exits per cell in Pass 3
```

**B. Indirect indexing prevents contiguous access.** Phosphor uses column-major
(`col * lines + line`) while the frame uses row-major (`line * width + col`).
Row-wise scans stride through non-contiguous phosphor memory, defeating SIMD
prefetching.

**C. `Color` enum is not SIMD-friendly.** The `crossterm::style::Color` has 18+
variants with mixed payloads. SIMD requires uniform layout — pre-decoding to
`(u8, u8, u8)` works for TrueColor but not for AnsiValue/Ansi16 without
branching.

**D. 10K cells cannot amortize SIMD overhead.** 10K cells = 1,250 AVX2 f32
iterations or 125 u8 iterations. Setup/teardown cost (lane alignment, mask
management, unpack) is proportionally significant.

### 4.2 What LLVM Auto-Vectorization Already Provides

With x86-64-v3, LLVM's SLP and loop vectorizers handle:
- `apply_brightness()` inner `r*f, g*f, b*f` (may fuse as FMA)
- `dist2()` independent i32 multiply-adds (quantization paths)
- Straight-line contiguous array arithmetic

LLVM *cannot* optimize (and manual SIMD also cannot): conditional branches,
enum dispatch, indirect indexing, and `f32::exp()`.

### 4.3 Estimated Impact

| Subsystem | Est. Improvement | Notes |
|-----------|-----------------|-------|
| Phosphor u8 decay | 2–4× for pure multiply | < 5% of frame time |
| Color f32 blending | 4–8× for parallel channels | < 3% of frame time |
| Frame `set()` | **None** | Logic-bound, not arithmetic |
| Droplet/Monolith draw | **None** | Per-cell conditional cascade |

**Net: 5–15%** of total frame time — translating to ~58–63 fps at 60 target,
imperceptible.

### 4.4 Cost of Manual SIMD

`unsafe` intrinsics (policy violation), architecture-specific fragility,
hard-to-test edge cases (alignment, overflow), aarch64 NEON port required,
`#[cfg(target_feature)]` gating complexity.

---

## 5. Detailed Function-Level Findings

### 5.1 `frame.rs::set()` — Highest Call Count, Zero SIMD Potential

~15K–25K calls/frame with 5+ conditional branches, indirect index calculation,
and BitVec/SmallVec side effects. **Logic-bound.** No arithmetic to vectorize.

### 5.2 `palette.rs` Blending — Arithmetic With Enum Dispatch

`color_to_rgb()` (lines 139–206): 7-way `match` on `Color`. Called 5–8×/cell.
`apply_brightness()` (lines 233–244): 3 f32 multiplies gated by enum dispatch
and u8↔f32 conversion. The `u8→f32→round→clamp→u8` chain defeats efficient
vectorization. AVX2 lacks a single-instruction round-to-nearest-u8 path.

### 5.3 `cloud/phosphor.rs::phosphor_decay_pass()` — Best SIMD Candidate

Three passes over all cells. The core arithmetic is `energy * (-rate*dt).exp()`.
The `exp()` call is scalar — neither LLVM nor manual SIMD (without AVX-512 ER)
can vectorize transcendental functions. A LUT approximation could enable batch
u8 processing, but gains are negligible at 10K cells.

### 5.4 `droplet.rs::viewport_edge_fade()` — Pure f32, Trivially Fast

3 branches per call but already `#[inline]`, compiles to ~10 instructions with
predictable branches. Interior cells (the vast majority) take the constant `1.0`
path.

### 5.5 `cloud/monolith.rs::draw_segments()` — Per-Cell Dispatch

`color_for_level()` (lines 665–717) branches on ColorMode, palette slot
presence, and BrightnessLevel (5-way match). Each cell takes a different path —
not vectorizable.

### 5.6 `cloud/spawn.rs` — RNG-Bound

Dominated by sequential `StdRng` sampling. RNG is inherently non-parallelizable.

---

## 6. Recommendation

### Decision: Do Not Introduce Manual SIMD

| Criterion | Assessment |
|-----------|-----------|
| Performance gain | 5–15% (imperceptible at 60 fps) |
| Implementation cost | High (unsafe intrinsics, arch gating) |
| Maintenance cost | High (compiler sensitivity, SIMD audits) |
| Policy compliance | **Violates no-new-unsafe renderer/core policy** |
| Benefit-to-cost ratio | **Unfavorable** |

### Rationale

1. **Performance is adequate** — stable 60 fps with comfortable headroom.
2. **LLVM already exploits the ISA** — auto-vectorizer handles vectorizable
   patterns; manual intrinsics add nothing for non-vectorizable code.
3. **Per-cell branching is fundamental** — the visual effects pipeline applies
   different effect combinations per cell by design.
4. **Workload is too small** — 10K cells = 1,250 AVX2 iterations; overhead
   is proportionally significant.
5. **Unsafe performance code is a policy violation** — the no-new-unsafe
   renderer/core invariant should not be broken for imperceptible gains.

### Recommended Alternatives

- **Palette pre-decoding**: Cache `color_to_rgb()` at construction time.
- **Cache-friendly layout**: Transpose phosphor to row-major to match frame.
- **Branchless arithmetic**: Replace `if factor <= 0.0` with `factor.max(0.0)`.
- **Adaptive skip**: Extend `perf_pressure` gating to more subsystems.

---

## 7. Future Conditions for SIMD Reconsideration

### 7.1 Workload Scale: 4K+ Terminals (8M+ Cells/Frame)

At 3840×2160 character cells (hypothetical): 8.3M cells, ~125–200 MFLOP/s.
SIMD would save 50–200μs/frame. No real terminal emulator supports this.

### 7.2 Data Layout: SoA with Row-Major Contiguous Storage

```rust
struct PhosphorRow {
    energy: [u8; MAX_COLS],  // contiguous → SIMD scan
    fresh:  [bool; MAX_COLS], // bitmask → SIMD mask
}
```

### 7.3 Branch Elimination: Homogeneous Cell Processing

Removing per-cell bloom/fog/glow/edge-fade branching via lookup tables or
baked constants. Requires accepting visual fidelity loss or multi-pass
architecture.

### 7.4 Policy Change: Accepting `unsafe` for Performance Paths

A formal `unsafe` policy (isolated modules, documented invariants) would
remove the cultural barrier. This is a project decision, not technical.

### 7.5 Decay Approximation

Replacing `f32::exp()` with a polynomial or LUT approximation in
`phosphor_decay_pass()` would enable full vectorization of the decay loop.
Saves ~10–20μs/frame at 10K cells — technically feasible without `unsafe`.

---

*End of document.*
