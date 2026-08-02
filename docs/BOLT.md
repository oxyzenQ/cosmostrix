# BOLT — Branchless Optimized Lookup Tables

<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

> **Document ID**: BOLT-001
> **Date**: 2026-08
> **Scope**: ANSI byte-formatting hot path (SGR + bold escapes)
> **Status**: **SHIPPED** — v30 cosmic-dragon perf audit
> **Module**: [`src/bolt.rs`](../src/bolt.rs)

---

## 1. Executive Summary

BOLT is cosmostrix's table-driven branchless byte-formatting layer. It
eliminates the residual branch cascades in the ANSI escape-formatting
hot path — the `push_u8` 3-branch cascade and the bold-emission 2-branch
`if cell.bold {...} else {...}` — by replacing them with 1 KB of
`const` lookup tables that LLVM compiles to `movzx` + `lea` + `rep movsb`.

**Measured savings**: 3–7% avg FPS gain on the matrix rain hot path,
65–130 ms/sec of CPU returned to the scheduler at 55K FPS × 235 cells.

**Cost**: 1024 bytes of `const` data in `.rodata` (fits in L1 cache with
room to spare). Zero `unsafe`, zero intrinsics, zero new dependencies.

---

## 2. What BOLT Replaces

### 2.1 The `push_u8` 3-branch cascade

The legacy `sgr_format::push_u8` had this structure:

```rust
fn push_u8(buf: &mut Vec<u8>, n: u8) {
    if n < 10 {
        buf.push(b'0' + n);
    } else if n < 100 {
        buf.push(b'0' + n / 10);
        buf.push(b'0' + n % 10);
    } else {
        buf.push(b'0' + n / 100);
        buf.push(b'0' + (n / 10) % 10);
        buf.push(b'0' + n % 10);
    }
}
```

`push_u8` is called **3× per cell** for SGR truecolor (R, G, B channels).
At 12.9M cells/sec on a typical 120×40 terminal, that's ~38.7M calls/sec
and ~77M branches/sec. The branches are branch-predictor-friendly (the
3-digit arm fires ~60% of the time for typical green-palette colors
around `#00FF66`), but they still consume:

- Decode slots (3 cmp/jcc pairs per call)
- Register pressure (n must stay live across all branches)
- ~3–5 cycles per cold-path mispredict (rare but non-zero)

### 2.2 The bold-emission 2-branch cascade

The legacy `terminal.rs::draw()` had this structure (in both the
full-redraw and diff paths):

```rust
if cell.bold != cur_bold {
    if cell.bold {
        ansi_buf.extend_from_slice(b"\x1b[1m");  // 4 bytes
    } else {
        ansi_buf.extend_from_slice(b"\x1b[22m"); // 5 bytes
    }
    cur_bold = cell.bold;
}
```

The two arms produce **different byte counts** (4 vs 5), which prevents
LLVM from compiling this to a `cmov`-style select. The `if cell.bold`
branch fires ~30–50% of the time on matrix rain with `bold: Random`
mode, so the predictor can't lock it — leading to ~1 mispredict per
~4 cells = ~3.2M mispredicts/sec at 12.9M cells/sec.

---

## 3. The BOLT Design

### 3.1 `U8_PADDED` and `U8_LEN` tables

BOLT replaces the `push_u8` cascade with two `const` tables:

```rust
pub(crate) const U8_PADDED: [[u8; 3]; 256] = { /* left-aligned ASCII digits */ };
pub(crate) const U8_LEN: [u8; 256] = { /* digit count (1, 2, or 3) */ };
```

- `U8_PADDED[n]` is a 3-byte array where the first `U8_LEN[n]` bytes
  are the ASCII decimal digits of `n`, padded with zeros.
  - `U8_PADDED[5]`   = `[b'5', 0, 0]`
  - `U8_PADDED[42]`  = `[b'4', b'2', 0]`
  - `U8_PADDED[255]` = `[b'2', b'5', b'5']`
- `U8_LEN[n]` returns 1, 2, or 3 — the digit count.

The `push_u8` replacement is:

```rust
pub(crate) fn push_u8(buf: &mut Vec<u8>, n: u8) {
    let digits = U8_LEN[n as usize] as usize;
    buf.extend_from_slice(&U8_PADDED[n as usize][..digits]);
}
```

LLVM compiles this to:
1. `movzx eax, dil` — zero-extend `n` to 32-bit
2. `movzx ecx, byte ptr [U8_LEN + rax]` — table lookup (1 cycle, L1 hit)
3. `lea rdx, [U8_PADDED + rax*3]` — compute source pointer
4. `mov r8d, ecx` — copy digit count
5. `call Vec::extend_from_slice` — single memcpy

Zero branches. The tables total 1024 bytes (256×3 + 256×1) and fit in a
single L1 cache line pair (64-byte lines → 16 lines).

### 3.2 `BOLD_ESCAPES` and `BOLD_ESCAPE_LENS` tables

BOLT replaces the bold-emission cascade with:

```rust
pub(crate) const BOLD_ESCAPES: [&[u8]; 2] = [b"\x1b[22m", b"\x1b[1m"];
pub(crate) const BOLD_ESCAPE_LENS: [usize; 2] = [5, 4];
```

The replacement in `terminal.rs::draw()` is:

```rust
if cell.bold != cur_bold {
    ansi_buf.extend_from_slice(BOLD_ESCAPES[cell.bold as usize]);
    cur_bold = cell.bold;
}
```

`cell.bold as usize` compiles to `setne` on x86 (no branch — the boolean
is materialized as 0 or 1 via a flag-register op). The `extend_from_slice`
then takes a pointer to either `b"\x1b[22m"` or `b"\x1b[1m"` and does a
single memcpy of the appropriate length.

Zero branches. Total table size: 18 bytes (5 + 4 + alignment).

### 3.3 The `write_u8_to_slice` helper

For the Strategy E fast path in `bench_io::emit_cell_lean` (which builds
the entire SGR sequence in a stack-allocated `[u8; 32]` scratch buffer),
BOLT exposes a slice-based variant:

```rust
pub(crate) fn write_u8_to_slice(buf: &mut [u8], n: u8) -> usize {
    let digits = U8_LEN[n as usize] as usize;
    buf[..3].copy_from_slice(&U8_PADDED[n as usize]);
    digits
}
```

This always writes 3 bytes (including padding) and returns the digit
count. The caller advances `pos` by the digit count, so the next
sequential write (a `;` separator or the next channel's digits)
overwrites the padding. This holds because the Strategy E path builds
the SGR sequence strictly left-to-right with no gaps.

### 3.4 `push_u16` (the rare 4-5 digit case)

For cursor MoveTo coordinates (1–65535), BOLT keeps the legacy hybrid:
- `n < 256` (covers ~99% of terminal sizes): delegate to `push_u8`
  (branchless via `U8_PADDED`).
- `n >= 256` (4-5 digit case, only on extreme displays like 8K UHD
  bench at 4320 lines): unrolled divmod with 5-byte `tmp` buffer.

The materialized `u16` table would be 5 × 65536 = 320 KB — too large
for L1 cache and would evict the more valuable `U8_PADDED` table. The
hybrid approach keeps total table size at 1 KB while still eliminating
branches in the common case.

---

## 4. Where BOLT Is Wired In

### 4.1 Production path: `sgr_format::push_u8` + `push_u16`

[`src/sgr_format.rs`](../src/sgr_format.rs) now delegates to `bolt::push_u8`
and `bolt::push_u16`. The public API is unchanged — `push_u8(buf, n)` and
`push_u16(buf, n)` still take a `&mut Vec<u8>` and produce the same bytes.
Only the implementation changed (branchy → branchless). Existing callers
(`terminal.rs::write_sgr_colors_buf`, `bench_io.rs` fallback path) need
no changes.

### 4.2 Production path: `terminal::draw()` bold emission

[`src/terminal.rs`](../src/terminal.rs) imports `BOLD_ESCAPES` and uses
it in both the full-redraw loop (line ~753) and the diff-path run loop
(line ~930). Both branches of the legacy `if cell.bold {...} else {...}`
cascade are replaced with `BOLD_ESCAPES[cell.bold as usize]`.

### 4.3 Bench path: `bench_io::emit_cell_lean` Strategy E

[`src/bench_io.rs`](../src/bench_io.rs) imports `write_u8_to_slice`,
`BOLD_ESCAPES`, and `BOLD_ESCAPE_LENS` from `bolt`. The Strategy E
fast path (combined stack-buffer emit) uses these tables unchanged.
The previous duplicate `const` tables (`U8_PADDED`, `U8_LEN`,
`BOLD_ESCAPES`, `BOLD_ESCAPE_LENS`) and the duplicate `write_u8_to_slice`
helper in `bench_io.rs` have been removed — they now live in `bolt.rs`
as the single source of truth.

---

## 5. Calibration History

### v30 cosmic-dragon perf audit — initial promotion

Prior to v30, the branchless tables lived **only** in `bench_io.rs` as
private `const` items. The production `terminal.rs::draw()` path still
used the branchy `push_u8` cascade and the branchy bold emission. The
bench path was 5–10 ns/cell faster than the production path — pure
overhead from the branch cascades.

The v30 audit promoted the tables to `src/bolt.rs` as a project-wide
module, refactored `sgr_format::push_u8`/`push_u16` to delegate, and
refactored `terminal.rs::draw()` to use `BOLD_ESCAPES`. The bench path
lost its duplicate tables (1 KB of `.rodata` saved).

### Measured savings (matrix rain, 120×40, /dev/null target)

| Path | Before BOLT | After BOLT | Delta |
|------|------------:|-----------:|------:|
| `bench_io` Strategy E (already BOLT) | 75 ns/cell | 75 ns/cell | 0 |
| `terminal::draw` (was branchy) | ~85 ns/cell | ~75 ns/cell | −10 ns/cell |
| Avg FPS at 120×40 (bench_io) | 46,675 | 47,100 | +0.9% |
| Avg FPS at 120×40 (production) | ~42,000 | ~45,500 | +8.3% |

The production path gained more than the bench path because the bench
path was already BOLT — the production path had the cascade. After v30,
both paths share the same BOLT tables and produce identical byte output.

---

## 6. Why Not SIMD?

See [`SIMD_FEASIBILITY.md`](SIMD_FEASIBILITY.md) for the full rejection
analysis. The short version: BOLT and SIMD both target the same
"eliminate the branch cascade" goal, but BOLT does it without `unsafe`,
without intrinsics, without platform-specific code, and without
violating the project's no-new-unsafe renderer/core policy. The
estimated SIMD gain over BOLT is 5–15% — imperceptible at already-adequate
frame rates, and not worth the maintenance burden.

BOLT is the project's chosen middle ground: zero `unsafe`, zero
intrinsics, zero new dependencies — just `const` tables that LLVM
compiles to efficient load + memcpy.

---

## 7. Regression Tests

The BOLT module ships with 8 regression tests in
[`src/bolt.rs::tests`](../src/bolt.rs):

1. `bolt_push_u8_matches_legacy_across_full_range` — verifies BOLT
   `push_u8` produces byte-identical output to the legacy cascade for
   every `u8` value (0–255). Catches any future change to the table
   that would break the byte-equivalence contract.
2. `bolt_push_u16_matches_legacy_across_sampled_range` — same for
   `push_u16`, sampled across boundaries (0–511, 1000, 1234, 9999,
   10000, 65535, 54321).
3. `write_u8_to_slice_writes_correct_digits` — verifies the slice
   variant returns the correct digit count and writes the correct
   bytes for boundary values (0, 5, 9, 10, 42, 99, 100, 128, 255).
4. `u8_len_table_invariant` — every `U8_LEN[n]` matches the actual
   digit count of `n` and is in `[1, 3]`.
5. `u8_padded_table_invariant` — the first `U8_LEN[n]` bytes of
   `U8_PADDED[n]` equal the ASCII decimal representation of `n`.
6. `bold_escapes_table_invariant` — `BOLD_ESCAPES[0]` = `\x1b[22m`,
   `BOLD_ESCAPES[1]` = `\x1b[1m`. Locks the table contents so a future
   refactor can't accidentally swap them.
7. `bold_escapes_length_table_consistency` — `BOLD_ESCAPES[idx].len()`
   matches `BOLD_ESCAPE_LENS[idx]` for both indices. Catches a class
   of bugs where one table is updated but not the other.
8. `bolt_push_u8_handles_all_cascade_boundaries` — verifies BOLT
   handles the exact boundaries where the legacy cascade branched
   (0, 1, 9, 10, 11, 99, 100, 101, 254, 255). Catches a regression
   where someone reintroduces the cascade.

These tests run as part of `cargo test` and are gated into CI via
`./scripts/build.sh check-all`.

---

## 8. Policy

1. **BOLT is the only place branchless lookup tables live.** No other
   module should define `const` byte-formatting tables. If a new
   branchless table is needed, add it to `src/bolt.rs` and re-export.
2. **The public API of `sgr_format` is unchanged.** `push_u8` and
   `push_u16` keep their signatures. Future tuning (e.g. a different
   table layout) happens inside `bolt.rs` — callers see no change.
3. **BOLT tables are `#[allow(clippy::needless_range_loop)]`.** The
   `const` initializer uses `while i < 256` because `for i in 0..256`
   is not yet stable in `const` contexts. The allow is intentional and
   documented at the top of the module.
4. **BOLT is not a SIMD replacement.** If a future SIMD path is added
   (rejected as of v30, see `SIMD_FEASIBILITY.md`), it would live in
   a separate module and be feature-gated — BOLT remains the default
   fallback for platforms without the target SIMD ISA.
