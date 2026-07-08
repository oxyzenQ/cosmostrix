<!-- SPDX-License-Identifier: GPL-3.0-only -->

<p align="center">
  <img src="assets/cosmostrix-logo.png" alt="cosmostrix logo" width="260">
</p>

<h1 align="center">cosmostrix</h1>

<p align="center">
  <strong>Production-grade cinematic Matrix rain renderer for serious terminal environments.</strong>
</p>

<p align="center">
  Engineered for smooth rendering, configurable atmosphere, clean terminal recovery, and reliable cross-platform operation.
</p>

<p align="center">
  <a href="https://ko-fi.com/rezky">
    <img src="https://img.shields.io/badge/Ko--fi-support-7C3AED?style=flat-square&logo=kofi&logoColor=white&labelColor=111827" alt="Support on Ko-fi">
  </a>
</p>

## Demo

<p align="center">
  <img src="assets/cosmostrix-v12-demo.gif" alt="Cosmostrix v12 masterclass demo" width="800">
</p>

<p align="center">
  <img src="assets/cosmostrix-v12-demo-cyberpunk.png" alt="Cosmostrix v12 cyberpunk charset demo" width="800">
  <br>
  <img src="assets/cosmostrix-v12-demo-retro.png" alt="Cosmostrix v12 retro charset demo" width="800">
  <br>
  <img src="assets/cosmostrix-v12-demo-braille.png" alt="Cosmostrix v12 braille charset demo" width="800">
  <br>
  <img src="assets/cosmostrix-v12-demo-hacker.png" alt="Cosmostrix v12 hacker charset demo" width="800">
  <br>
  <img src="assets/cosmostrix-v12-demo-blocks.png" alt="Cosmostrix v12 blocks charset demo" width="800">
</p>

<p align="center">
  <a href="https://www.youtube.com/watch?v=KSk-DWFdg3A">YouTube</a>
</p>

Signature Monolith Rain, cinematic themes, and message mode in a real terminal session.

## Features

- **Cinematic terminal rain** — calm, organic, premium visual feel with crisp head/body/trail hierarchy and desynchronized column speeds (async mode default ON for organic feel)
- **3 scene atmospheres** (matrix, monolith, signal), including signature Cosmostrix Monolith Rain
- **8 curated presets** (classic, cinematic, calm, monolith, storm, cosmos, neon, hacker) for one-command visual profiles
- 43 built-in themes and 24 character set presets (5 themes — Green3, Saturn, Comet, Meteor, Pluto — re-tuned in v11.1.0 for visual distinctness; `--color-tune` turns all 43 into 43 × ∞ variants)
- Phosphor persistence (CRT afterglow), depth fog, and 3-layer parallax
- TrueColor green gradients with luminous head glow
- Configurable speed, density, FPS, and glitch intensity
- Alternate screen with diff-based rendering — no scrollback spam
- Adaptive throttling: reduces CPU usage when idle
- Screensaver mode
- Optional mouse hover/click effects (`--mouse`)
- Safe terminal cleanup and recovery (`--reset-terminal`)
- Cross-platform: Linux, macOS, Windows, Android (Termux)

## Requirements

- Rust stable toolchain to build from source
- A terminal supporting ANSI escape sequences, alternate screen, and raw mode
- Best results with 256-color or truecolor terminals

## Installation

### GitHub Releases (prebuilt binaries)

Download from [Releases](https://github.com/oxyzenQ/cosmostrix/releases), verify the checksum, and place `cosmostrix` in your `PATH`.

Each release ships **three** checksums: classical SHA-512 + quantum-resistant
BLAKE2b-512 + SHAKE256. Full instructions in
[docs/VERIFY_RELEASE.md](docs/VERIFY_RELEASE.md).

```bash
# Classical (universal)
sha512sum -c cosmostrix-vX.Y.Z-linux-amd64-musl.tar.gz.sha512sum

# Quantum-resistant — BLAKE2b (fastest, in coreutils)
b2sum -c cosmostrix-vX.Y.Z-linux-amd64-musl.tar.gz.b2sum

# Quantum-resistant — SHAKE256 (NIST PQ standard, via Python)
# openssl's -shake256 default output length varies; Python is consistent
COMPUTED=$(python3 -c "import hashlib; print(hashlib.shake_256(open('cosmostrix-vX.Y.Z-linux-amd64-musl.tar.gz','rb').read()).hexdigest(64))")
EXPECTED=$(awk '{print $1}' cosmostrix-vX.Y.Z-linux-amd64-musl.tar.gz.shake256)
[ "$COMPUTED" = "$EXPECTED" ] && echo "OK" || echo "FAILED"
```

**Available platforms:**

- Linux amd64: `v3`, `v4`, `musl` (also `linux-aarch64` for arm64)
- macOS: `darwin-aarch64-native` (Apple Silicon)
- Windows: `windows-x86_64`, `windows-aarch64-native`
- Android (Termux): `android-aarch64-native`

```bash
REPO="oxyzenQ/cosmostrix"
TAG="v12.0.0"
PLATFORM="linux-amd64-v3"
curl -LO "https://github.com/${REPO}/releases/download/${TAG}/cosmostrix-${TAG}-${PLATFORM}.tar.gz"
curl -LO "https://github.com/${REPO}/releases/download/${TAG}/cosmostrix-${TAG}-${PLATFORM}.tar.gz.sha512sum"
sha512sum -c "cosmostrix-${TAG}-${PLATFORM}.tar.gz.sha512sum"
tar -xzf "cosmostrix-${TAG}-${PLATFORM}.tar.gz"
./cosmostrix -i
```

### AUR (Arch Linux)

```bash
paru -S cosmostrix-bin    # or: yay -S cosmostrix-bin
```

### From source

```bash
git clone https://github.com/oxyzenQ/cosmostrix.git
cd cosmostrix
cargo install --path .
cosmostrix -i
```

### Optimized local builds

For a modern Linux x86_64 machine, the recommended optimized build is:

```bash
cargo pro-linux-v3
```

Artifact variants use explicit CPU baselines:

| Variant | Baseline |
|---|---|
| `linux-amd64-v3` | AVX2 / BMI2 / FMA-era CPUs (2013+, most modern x86_64) |
| `linux-amd64-v4` | AVX-512 baseline (high-end server/workstation) |
| `linux-amd64-musl` | v3 baseline + statically linked (max portability) |
| `native` | Local-only build tuned for the current CPU |

> **Note:** v1/v2 x86_64 variants were dropped in v11.0.0. Modern CPUs
> (2013+) support v3. For maximum portability (Alpine, containers,
> minimal base images), use the `musl` variant — it's statically linked
> with no glibc dependency.

Release/pro builds keep `panic = "unwind"` on purpose. Cosmostrix owns raw mode,
alternate screen, cursor visibility, and line-wrap state while running; unwinding
lets the RAII terminal guard and panic hook restore the terminal on panic.

To verify an optimized artifact:

```bash
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix -i
file target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix
scripts/verify-release-build.sh pro-linux-v3
```

## Quickstart

```bash
cosmostrix                           # signature Monolith Rain default
cosmostrix --color rainbow --speed 12   # color + speed
cosmostrix --screensaver              # exit on keypress
cosmostrix --message "wake up, neo"   # overlay message
cosmostrix --charset katakana         # character set
cosmostrix --preset cinematic          # curated preset
cosmostrix --scene monolith --color deepspace
cosmostrix --config ./cosmostrix.conf  # explicit config file
cosmostrix --profile nightcore         # user-defined config profile
```

## CLI Reference

Run `cosmostrix --help` for common options or `cosmostrix --help-detail` for the full reference.

```text
COMMON OPTIONS
  -c, --color <name>        Color theme
     --color-tune <k=v>     Tune saturation/brightness (e.g. saturation=1.5,brightness=0.9)
     --charset <name>       Character preset
  -f, --fps <1-240>         Target FPS
  -S, --speed <1-100>       Rain speed
  -d, --density <0.01-5.0>  Rain density
  -s, --screensaver         Exit on keypress
     --mouse                Enable mouse hover/click effects
  -m, --message <text>      Overlay message
     --low-power            Power-saving mode
     --glitch-level <level> Glitch intensity (none|subtle|default|intense)
     --preset <name>       Apply a named preset
     --scene <name>        Apply a scene atmosphere
     --profile <name>      Apply a user-defined config profile
     --config <path>        Load config from an explicit file
     --dump-config          Print an example config and exit

DIAGNOSTICS
     --doctor               Compatibility report
     --benchmark            Renderer benchmark (5s default; override with --bench-duration)
     --bench-duration <1-600>  Benchmark duration in seconds (for long-run drift / leak detection)
     --json                 Output benchmark as JSON (use with --benchmark; for CI/scripts)
  -i, --info                Build and runtime information
     --reset-terminal       Destructive terminal recovery (clears screen + scrollback)

DISCOVERY
     --list-colors          Show compact color theme names
     --list-charsets        Show available charset presets
     --list-presets         Show available presets
     --list-scenes          Show available scene atmospheres
     --defaults             Show the default runtime profile
```

Explicit CLI flags always override preset, scene, and profile values.

## Runtime Controls

```text
  q / Esc       Quit              p          Pause / resume
  c / C         Cycle theme       s / S      Cycle charset
  x / X         Cycle scene       [ / ]      Density
  Up / Down     Speed             g          Toggle glitch
  m             Cycle profile     Space      Reseed animation
  ?             Toggle live HUD (FPS / p99 / max / RSS overlay)
```

## Scenes

- `matrix` — classic Matrix glyph rain
- `monolith` — default signature Cosmostrix Monolith Rain with sparse structured segments
- `signal` — digital transmission / code-signal atmosphere

Press `x` or `X` while running to cycle scenes forward: Monolith Rain → Matrix → Signal → Monolith.

## Configuration

Persistent defaults can be set in `~/.config/cosmostrix/config` (or `$XDG_CONFIG_HOME/cosmostrix/config`). Use `--config <path>` to load a specific file.

```
scene = monolith
preset = cinematic
color = cosmos
charset = binary
fps = 60
speed = 20
density = 0.75
glitch-level = subtle
mouse = false
```

Precedence: defaults → config file → preset/scene/profile layers → explicit CLI flags.

```bash
cosmostrix --dump-config        # print example config
cosmostrix --list-profiles      # list user profiles
cosmostrix --config-path        # print default config path
```

## Terminal Recovery

Quit with `q`, `Esc`, or Ctrl+C when possible. If a terminal is left in raw mode or alternate screen:

```bash
cosmostrix --reset-terminal
```

On Windows PowerShell: `.\cosmostrix.exe --reset-terminal`

For terminal behavior, background modes, tmux/SSH notes, and Windows recovery expectations, see [Terminal Compatibility](docs/TERMINAL_COMPATIBILITY.md).

## Benchmarking

Benchmark results are machine-dependent. Use them to compare builds on the same machine, not as portable performance promises. Optimized builds remain comfortably above the 60 FPS target.

```bash
cargo pro-linux-v3
COSMOSTRIX_BENCH_COLS=120 COSMOSTRIX_BENCH_LINES=40 \
  target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --benchmark
```

The `--benchmark` report (v11.1.0+) includes FPS, frame-time percentiles
(avg → p95 → p99 → p99.9 → max), MEMORY (RSS), CPU usage %, sub-component
timing (sim/render/io), and a DRIFT section for long-run analysis. The
SYSTEM section records the CPU model, rustc version, LTO/PGO status, and
git SHA so reports are self-documenting for cross-machine comparison. A
RESOURCE section reports page faults + context switches via `getrusage`.
A BENCHMARK ENVIRONMENT section records kernel, libc, terminal, CPU
governor, and SMT status for reproducibility. The RENDERER section
explicitly declares `gpu_usage: not_applicable` — cosmostrix is a CPU +
stdout renderer, no GPU context is ever created.

**Benchmark mode measures the engine without writing to the terminal.**
FPS numbers are synthetic uncapped throughput — how many frames the
renderer can *compute* per second, not how many frames the terminal
*draws*. Real interactive FPS is bounded by the terminal emulator,
refresh rate, and ANSI output bandwidth. Use `?` (live HUD) during a
real run to see actual interactive FPS.

Use `--bench-duration N` (1–600s) for sustained drift / leak detection:

```bash
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --benchmark --bench-duration 60
```

Use `--json` for machine-readable output (CI/scripts):

```bash
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --benchmark --json | jq .performance.avg_fps
```

See [benchmark/README.md](benchmark/README.md) for full reference results and interpretation notes.

## Documentation

- [Changelog](CHANGELOG.md) — release history
- [Terminal Compatibility](docs/TERMINAL_COMPATIBILITY.md) — terminal behavior, tmux/SSH, recovery
- [Visual Stability](docs/VISUAL_STABILITY.md) — visual depth and throughput stability
- [Endurance](docs/ENDURANCE.md) — endurance testing and resource monitoring
- [Atmosphere Engine](docs/ATMOSPHERE_ENGINE.md) — atmosphere and whisper engine internals
- [Supply Chain](docs/SUPPLY_CHAIN.md) — supply-chain hardening policy
- [Stability Audit](docs/STABILITY_AUDIT.md) — terminal stability audit
- [SIMD Feasibility](docs/SIMD_FEASIBILITY.md) — SIMD optimization feasibility
- [CI & Release Workflow](docs/workflow/about-ci.md) — CI pipeline and release process

## Development

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --all --locked
scripts/verify-release-build.sh pro-linux-v3 pro-linux-v4 pro-linux-musl
```

## Release Process

Create a release by pushing a `v*` tag. See [docs/workflow/about-ci.md](docs/workflow/about-ci.md) for CI and release workflow details.

## Contributing

PRs and issues are welcome. Please run `cargo fmt` and `cargo clippy` before submitting. See [RULES.md](docs/RULES.md) for project conventions.

## Support

cosmostrix is an open-source project built and maintained independently by [rezky_nightky (oxyzenQ)](https://github.com/oxyzenQ).

If this project helped you, or saved development time, you can support future maintenance here:

[![Support me on Ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/rezky)

Support is optional. The project remains open-source.

## Intellectual Property & Trademark

**cosmostrix** is the exclusive intellectual property of **rezky_nightky (oxyzenQ)**. Source code: **GPL-3.0-only** (see [LICENSE](LICENSE)). Name, logo, and branding ("the Marks") are governed by [TRADEMARK.md](TRADEMARK.md), are NOT covered by the GPL, and are reserved by the owner. This project is **NOT for sale**; unauthorized rebranding, relicensing, or source-code theft is strictly prohibited. For trademark licensing or written permission, contact **rezky_nightky (oxyzenQ)** — https://github.com/oxyzenQ.
© 2026 rezky_nightky (oxyzenQ). All rights reserved.
