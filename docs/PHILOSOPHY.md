# Cosmostrix Rendering Philosophy

> Cosmostrix is a CPU-only terminal renderer by design. The terminal is a text
> medium — its soul is ANSI escape sequences, copy-pasteable glyphs, and the
> slow poetry of a phosphor decay. A GPU would paint an image; Cosmostrix
> writes a sentence.

This document consolidates the architectural philosophy that governs
Cosmostrix's rendering decisions. It is the canonical reference for why
certain features are rejected, why the renderer is single-threaded, and
why the terminal — not a framebuffer — is the canvas.

## 1. CPU-Only, Forever

Cosmostrix never opens a GPU context. No OpenGL, Vulkan, Metal, DirectX, or
WebGPU handle is ever created. The benchmark reports `gpu_usage: not_applicable`,
and `--info` carries the same field for consistency.

### Why

1. **The terminal IS text.** Cosmostrix emits ANSI escape sequences (SGR for
   color, cursor addressing, alternate screen) via `crossterm`. The terminal
   emulator owns the framebuffer; a CLI process has no legal way to write
   pixels to it. The backend is `ansi-stream` — that is the architectural
   truth.

2. **The bottleneck is the terminal, not the renderer.** The terminal
   emulator's ANSI parse speed is the ceiling. No amount of SIMD, GPU, or
   C supercharger can fix a slow terminal — it is a separate process
   Cosmostrix cannot control. A GPU on Cosmostrix's side would still wait
   on Alacritty/kitty's ANSI parser.

3. **GPU rendering requires an image protocol** (kitty `ESC_G`, Sixel),
   which produces an image, not text. This would change Cosmostrix from
   "terminal rain" to "image rain" — a different program with a different
   soul. The character-grid aesthetic (you can still select and copy rain)
   would be lost.

4. **Terminal emulator support for image protocols is fragmentary.** Only
   kitty, wezterm, foot support kitty graphics. gnome-terminal, xterm,
   Termux, Windows conhost — no. A GPU path would silently fail or fork
   the user experience. Cosmostrix's cross-platform matrix (Linux, macOS,
   Windows, Android/Termux) depends on universal ANSI, not
   protocol-of-the-month.

5. **No framebuffer access from a normal CLI process.** Even ignoring
   protocols, writing to `/dev/fb0` or DRI requires permissions Cosmostrix
   does not have and should not request. The crossterm abstraction is the
   right ceiling.

### The "Never" is Scoped

The word "never" applies to **the main branch and the default renderer**.
The DRAGON exploration docs (`docs/DRAGON_EXPLORATION.md §3.4`) honestly
acknowledge that GPU image-mode is *technically possible* as an "art
mode" — but it would be a different program. If it ever shipped, it would
be a separate `cosmostrix-image` companion binary using kitty graphics,
not a code path in this binary.

## 2. Single-Threaded, Single-Owner Writer

The terminal is a single-writer device; parallelism does not help.
Cosmostrix embraces this — `compute_parallelism: disabled` is an
architectural invariant, not a missing feature.

Adding threads would complicate the panic-recovery, signal-handling, and
terminal-cleanup machinery without improving throughput. The bottleneck is
PTY bandwidth and terminal ANSI parse speed, both of which are external to
Cosmostrix.

## 3. No Manual SIMD Intrinsics

LLVM auto-vectorization under `x86-64-v3` (AVX2) is active. Manual SIMD
intrinsics were evaluated in `docs/SIMD_FEASIBILITY.md` and explicitly
rejected:

- 5–15% gain (marginal)
- Requires `unsafe` blocks in the renderer hot path (violates the
  no-unsafe-in-renderer policy)
- Platform-specific maintenance burden (AVX2, AVX-512, NEON, SVE)

The auto-vectorizer already captures most of the win without the cost.

## 4. Scene-Naming Honesty

Scene names like `gpu-accelerated`, `4k-ready`, or `ultra-fast` are
**forbidden** (see `docs/CINEMATIC_BREATHING.md §Scene Naming Contract`).
They imply performance characteristics that depend on the user's hardware
and terminal. Scenes are named for their *visual character* (monolith,
storm, calm, cosmos), not their performance tier.

## 5. Visual Identity Locked

Per `docs/ROADMAP.md`, the visual identity must remain identical to
v3.9.0. The rain is character-grid text, not photorealistic images. This
is a non-negotiable invariant — any change that breaks the character-grid
aesthetic is rejected, regardless of the performance or visual gain.

## 6. Honesty as a Release Gate

The `docs/RELEASE_GUARD.md` "Honesty Rules" section mandates honesty as a
release gate:

- `SIGKILL` cleanup cannot be guaranteed — documented, not hidden
- `50k FPS` is not a release promise — it is a headless ceiling
- Renderer invariants are non-negotiable

The `--doctor` command prints `sigkill: cannot be caught or guaranteed` to
every user who runs it. The README "Limitations" section consolidates
these honestly. Power users respect this; they are suspicious of projects
that claim zero limitations.

## See Also

- [DRAGON_EXPLORATION.md](DRAGON_EXPLORATION.md) — §3.4 GPU Offload (rejected)
- [DRAGON_FINDINGS.md](DRAGON_FINDINGS.md) — terminal bottleneck analysis
- [SIMD_FEASIBILITY.md](SIMD_FEASIBILITY.md) — manual SIMD rejection
- [CINEMATIC_BREATHING.md](CINEMATIC_BREATHING.md) — scene-naming contract
- [RELEASE_GUARD.md](RELEASE_GUARD.md) — honesty rules
- [RENDER_ENGINE.md](RENDER_ENGINE.md) — renderer architecture
