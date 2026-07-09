# Dragon Experimental — Future Technology Explorations

<!-- SPDX-License-Identifier: GPL-3.0-only -->

> **Branch**: `dragon-experimental`
> **Status**: research / prototyping / NOT for production
> **Purpose**: explore bleeding-edge technologies to push cosmostrix
> beyond v13.3.0's 28K FPS headless ceiling.

This document is an honest engineering assessment of "wild idea"
technologies for cosmostrix's future. It separates **physically possible**
from **physically impossible**, and proposes real experimental directions.

---

## 1. The 1 Million FPS Question

**Can cosmostrix hit 1,000,000 FPS?**

**Short answer**: No. Physics forbids it for a terminal renderer.

**Long answer**: The bottleneck is NOT the engine. cosmostrix already
computes 28,000 FPS headless (v13.3.0). The limits are:

### Limit 1: Monitor refresh rate (hard ceiling)

A 240 Hz monitor displays 240 FPS. 1,000,000 FPS is 4,166× faster than
the monitor can show. The extra 999,760 frames per second are computed
but never displayed. They're pure waste.

Even a theoretical 1000 Hz monitor (does not exist commercially) caps
at 1000 FPS. 1M FPS requires a 1 MHz monitor — physically impossible
with current display technology (HDMI 2.1 maxes at 240 Hz at 4K).

### Limit 2: PTY pipe throughput

A PTY (pseudo-terminal) has a kernel buffer of ~64 KiB. Once full,
`write()` blocks. At 48 KB/frame (naive full redraw), you can buffer
~1.3 frames before blocking. Even with cosmostrix's 7 KB/frame
(diff-based + RLE), you buffer ~9 frames.

The terminal emulator drains this pipe at its parse speed:
- Alacritty (fastest): ~500,000 ANSI bytes/second
- kitty: ~400,000 bytes/second
- gnome-terminal: ~200,000 bytes/second

At 7 KB/frame and 500 KB/s drain rate: max sustainable = **71 FPS**
(Alacritty). The pipe throttles cosmostrix to the terminal's speed.

### Limit 3: Terminal emulator parse speed

Even if the pipe were infinite, the terminal emulator must parse each
ANSI escape sequence, update its internal grid, and render to screen.
Alacritty (Rust, GPU-accelerated) can parse ~500K bytes/s. That caps
interactive FPS at ~71 (7 KB/frame) to ~10 (48 KB/frame).

### Verdict

**1M FPS is impossible for a terminal renderer.** The engine could
compute it (28K FPS headless → could be pushed to 100K+ with SIMD),
but the terminal + monitor + PTY bottleneck caps interactive FPS at
~60-240 depending on hardware.

The honest target is: **maximize headless FPS** (engine ceiling) and
**minimize bytes/frame** (bandwidth). cosmostrix already does both well.

---

## 2. eBPF — What It Can and Cannot Do

**eBPF** (extended Berkeley Packet Filter) runs sandboxed programs in
the Linux kernel. It's powerful but widely misunderstood.

### What eBPF CAN do for cosmostrix

**1. Nanosecond profiling (valuable)**

Attach uprobes (user-space probes) to cosmostrix hot functions:
- `frame::set()` — cell equality check
- `terminal::draw()` — diff path + RLE encoding
- `color_cache::sgr_for_cell()` — cache lookup
- `flush_ansi()` — write to stdout

eBPF captures entry/exit timestamps with ~50ns overhead per probe.
This reveals exactly where frame time is spent, without modifying
cosmostrix's code. More precise than `--perf-stats` (which uses
`Instant::now()` at ~20ns but only at function boundaries).

**Tool**: `bpftrace` one-liners:
```bash
# Profile frame.set() call frequency + duration
bpftrace -e 'uprobe:/path/to/cosmostrix:_ZN10cosmostrix5frame5Frame3set17... 
  { @start = nsecs; }
  uretprobe:/path/to/cosmostrix:_ZN10cosmostrix5frame5Frame3set17...
  { @dur = hist(nsecs - @start); }'
```

**2. Syscall tracing**

Trace `write()` syscalls to stdout, measure bytes per call + latency:
```bash
bpftrace -e 'tracepoint:syscalls:sys_enter_write /comm=="cosmostrix"/ 
  { @bytes = hist(args->count); }'
```

This would reveal if `flush_ansi()` is doing too many small writes vs
one big write (it should be one big write — verify with eBPF).

### What eBPF CANNOT do

**1. "Bypass CPU limits"** — NO.

eBPF programs run in kernel context. They cannot:
- Speed up userspace code
- Bypass CPU clock speed
- Inject "extra" performance
- Override the scheduler

eBPF is a **measurement tool**, not a **speed boost**. Thinking eBPF
can "bypass limits" is a fundamental misunderstanding.

**2. "Inject performance"** — NO.

There is no kernel API to "inject" CPU cycles or memory bandwidth.
The kernel scheduler allocates CPU time fairly. eBPF can't override
this — it would be a security nightmare.

**3. Replace the rendering engine** — NO.

eBPF programs can't render to a terminal. They run in kernel space,
can't access userspace libraries (crossterm, etc.), and can't write
to stdout directly. They're for observability.

### Verdict

**eBPF is valuable for profiling cosmostrix**, not for boosting FPS.
A `scripts/ebpf-profile.sh` that attaches uprobes to hot functions
and reports a flame graph would be a real contribution. But it won't
make cosmostrix faster — it'll show us WHERE to optimize next.

---

## 3. C Supercharger — What's Real

The idea of a `supercharger.c` that "bypasses limits to boost FPS"
is based on a misunderstanding. The limits are physical (CPU, memory
bandwidth, PTY, monitor). A C file can't bypass them.

BUT, there ARE legitimate C-level optimizations that could boost the
**engine ceiling** (headless FPS):

### 3.1 SIMD Cell Comparison (real, 2-3× speedup)

**Current**: `Cell` is 16 bytes. `frame.set()` compares cells with
derived `==` (compiler emits scalar byte-wise compare, ~4 cycles).

**SIMD**: pack 1 cell into 1 `__m128i` register. Compare with
`_mm_cmpeq_epi8` (1 cycle). Compare 2 cells simultaneously with AVX2
`_mm256_cmpeq_epi8` (1 cycle for 32 bytes = 2 cells).

```c
// supercharger.c — SIMD cell equality (x86-64 SSE2)
#include <emmintrin.h>
#include <stdbool.h>
#include <stdint.h>

// Cell layout: ch(4) + fg(4) + bg(4) + bold_flags(4) = 16 bytes
typedef struct __attribute__((packed)) {
    uint32_t ch;
    uint32_t fg;  // packed RGB + is_set flag
    uint32_t bg;  // packed RGB + is_set flag
    uint32_t flags; // bold + padding
} cell16_t;

// Compare 1 cell — 1 cycle vs 4 cycles scalar
static inline bool cell_eq_simd(const cell16_t* a, const cell16_t* b) {
    __m128i va = _mm_loadu_si128((const __m128i*)a);
    __m128i vb = _mm_loadu_si128((const __m128i*)b);
    __m128i cmp = _mm_cmpeq_epi8(va, vb);
    return _mm_movemask_epi8(cmp) == 0xFFFF;
}

// Compare 2 cells simultaneously (AVX2) — 0.5 cycle per cell
#include <immintrin.h>
static inline uint32_t cells_eq2_avx2(const cell16_t* a, const cell16_t* b) {
    __m256i va = _mm256_loadu_si256((const __m256i*)a);
    __m256i vb = _mm256_loadu_si256((const __m256i*)b);
    __m256i cmp = _mm256_cmpeq_epi8(va, vb);
    return _mm256_movemask_epi8(cmp); // 0xFFFFFFFF = both equal
}
```

**Impact**: `frame.set()` hot path goes from ~4 cycles to ~1 cycle per
cell. At 28K FPS × 4800 cells/frame = 134M comparisons/s. Saving 3
cycles each = 402M cycles/s saved = ~130ms/s of CPU time freed.

**Catch**: cosmostrix's `Cell` uses `Option<Color>` (niche-optimized to
4 bytes), not packed RGB. Would need to refactor Cell layout. The
early-exit on `ch` field (first 4 bytes) already short-circuits most
comparisons, so real-world gain is <10%, not 4×.

### 3.2 io_uring Output (real, reduces syscall overhead)

**Current**: `flush_ansi()` does `BufWriter::write_all()` → eventually
`write()` syscall. Syscall overhead: ~1µs each.

**io_uring**: Linux 5.1+ async I/O. Submit writes to a kernel ring
buffer, kernel completes them asynchronously. Overhead: ~100ns per
submission (10× less than syscall).

```c
// supercharger.c — io_uring output (Linux 5.1+)
#include <liburing.h>
#include <unistd.h>

typedef struct {
    struct io_uring ring;
    int stdout_fd;
} uring_output_t;

void uring_init(uring_output_t* o) {
    io_uring_queue_init(8, &o->ring, 0);
    o->stdout_fd = STDOUT_FILENO;
}

// Submit a write without blocking — returns immediately
void uring_write(uring_output_t* o, const uint8_t* buf, size_t len) {
    struct io_uring_sqe* sqe = io_uring_get_sqe(&o->ring);
    io_uring_prep_write(sqe, o->stdout_fd, buf, len, 0);
    io_uring_submit(&o->ring);
    // Kernel will complete this asynchronously
}
```

**Impact**: at 60 FPS, 60 writes/s. Syscall overhead = 60µs/s.
io_uring = 6µs/s. Saves 54µs/s = negligible. **Not worth it** at
cosmostrix's current frame rate.

**When it matters**: if cosmostrix ever targets 1000+ FPS (custom
terminal), io_uring saves 1ms/s. Still small. io_uring is for
high-IOPS workloads (databases, network servers), not terminal renderers.

### 3.3 Shared-Memory Terminal Protocol (theoretical, huge payoff)

**Current**: cosmostrix → PTY pipe → terminal emulator parses ANSI →
terminal renders. Two context switches per frame, ANSI parsing overhead.

**Shared-memory**: cosmostrix mmaps a shared region with the terminal
emulator. Writes cell data directly to terminal's frame buffer. No pipe,
no ANSI parsing, no context switch.

```c
// supercharger.c — shared memory frame buffer (theoretical)
#include <sys/mman.h>
#include <sys/shm.h>

typedef struct {
    uint16_t width;
    uint16_t height;
    cell16_t cells[]; // flexible array
} shared_frame_t;

// cosmostrix side: create shared memory
shared_frame_t* shared_frame_create(uint16_t w, uint16_t h) {
    size_t size = sizeof(shared_frame_t) + w * h * sizeof(cell16_t);
    int shmid = shmget(IPC_PRIVATE, size, IPC_CREAT | 0666);
    shared_frame_t* frame = shmat(shmid, NULL, 0);
    frame->width = w;
    frame->height = h;
    return frame;
}
```

**Catch**: NO terminal emulator supports this. You'd need to:
1. Write a custom terminal emulator that reads the shared frame
2. Or convince Alacritty/kitty to add support (they won't — it's a
   massive protocol change for a niche use case)

**Verdict**: theoretically interesting, practically a 10-year project.
Not worth pursuing unless you're building cosmostrix + custom terminal
as a bundle.

### 3.4 GPU Offload via Graphics Protocol (real, loses terminal feel)

**Current**: cosmostrix renders text cells, terminal displays them.

**GPU**: cosmostrix renders to a Vulkan compute shader, sends the
resulting image via kitty graphics protocol (ESC_G) or Sixel.

```rust
// Pseudocode — GPU rain rendering
let vulkan_instance = vulkan::Instance::new()?;
let shader = compile_glsl(rain_compute_shader_src)?;
let output_image = shader.dispatch(width, height, rain_params)?;
terminal.send_kitty_graphics(output_image)?;
```

**Impact**: pixel-perfect rendering, true Gaussian blur for
depth-of-field, custom fonts, sub-pixel antialiasing. Could hit
1000+ FPS on GPU.

**Catch**:
- Terminal support: kitty, wezterm, foot only. gnome-terminal, xterm,
  Termux = no.
- Feel: it's an image, not terminal text. Loses the "I can copy-paste
  the rain" aesthetic.
- Complexity: Vulkan is 2000+ lines of setup code. Shader debugging is
  painful.

**Verdict**: interesting for an "art mode" but changes cosmostrix's
identity from "terminal rain" to "image rain". Not for main branch.

---

## 4. Real Experimental Directions for dragon-experimental

Based on the above, here are directions that are **actually worth
exploring** on this branch:

### 4.1 SIMD Cell Comparison (P1 — real, measurable)

- Refactor `Cell` to 16-byte packed layout (ch:u32, fg:u32, bg:u32, flags:u32)
- Add `cell_eq_simd()` in a C file, call via FFI
- Benchmark: does headless FPS go from 28K to 35K+?
- Risk: niche optimization on `Option<Color>` ergonomics

### 4.2 eBPF Profiling Script (P1 — zero risk, high insight)

- `scripts/ebpf-profile.sh` using bpftrace
- Attach uprobes to `frame::set`, `terminal::draw`, `flush_ansi`
- Output: flame graph + hot function timing
- Reveals the NEXT bottleneck after v13.3.0

### 4.3 Per-Row Hash Fast Path (P2 — idle mode win)

- 64-bit hash per row, updated incrementally on `set()`
- Before `draw()`: if row hash == previous frame, skip entire row
- Saves iteration in idle/sustained-still scenes
- Risk: hash maintenance overhead may exceed savings in active mode

### 4.4 io_uring Output (P3 — low ROI at 60 FPS)

- Replace BufWriter with io_uring submission queue
- Only worth it if targeting 500+ FPS (custom terminal scenario)
- Low priority for cosmostrix's 60 FPS target

### 4.5 Custom Terminal Protocol Research (P4 — long-term)

- Document what a shared-memory terminal protocol would look like
- Not implementation — just design doc + feasibility analysis
- If compelling, could spin off as separate project

---

## 5. What Will NOT Be on This Branch

- **"1 million FPS" claims** — physically impossible, won't fake it
- **eBPF "performance injection"** — eBPF doesn't work that way
- **C code that "bypasses CPU limits"** — no such thing exists
- **GPU rendering as default** — changes cosmostrix's identity

This branch is for honest engineering experiments, not marketing claims.

---

## 6. Branch Conventions

- **No merges to main** without explicit review + data proving the
  change is beneficial
- **All experiments must be benchmarked** before/after with
  `--benchmark --json` and `--perf-stats`
- **Honest documentation** — if an experiment fails, document why
- **C code goes in `src/supercharger.c`** with a Rust FFI wrapper
- **eBPF scripts go in `scripts/ebpf-*.sh`**

---

## 7. First Experiment: SIMD Cell Compare

The most promising P1 item. Plan:

1. Create `src/supercharger.c` with `cell_eq_simd()` (SSE2)
2. Create `src/supercharger.rs` FFI wrapper
3. Refactor `Cell` to 16-byte packed layout (or add a parallel `Cell16`)
4. Benchmark `--benchmark --json` before/after
5. If headless FPS improves by >5%, keep. Else, document why and revert.

This is the honest path to "supercharging" cosmostrix — not bypassing
physics, but optimizing the hot path with real CPU instructions.
