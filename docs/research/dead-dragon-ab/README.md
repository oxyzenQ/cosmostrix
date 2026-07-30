# Cosmic Dragon vs Dead Dragon — A/B Benchmark

**Date:** 2026-07-30
**Cosmic Dragon commit:** `6986f42` (branch `main`)
**Dead Dragon commit:** `aacce5d` (branch `dead-dragon`)

## What is this?

An A/B benchmark that proves the Cosmic Dragon diff-based rendering engine
(`src/frame.rs` + `src/terminal.rs` + `src/runtime.rs`) outperforms a
primitive full-redraw engine by a margin that scales with screen size.

The **Dead Dragon** is a surgical mutation of the Cosmic Dragon: same
visual output, same rain simulation, same benchmark harness — but the
diff-based renderer is crippled so every frame redraws every cell from
scratch. It exists purely as a control subject.

## Files

| File | Description |
|---|---|
| [`comparison-report.md`](comparison-report.md) | Human-readable Markdown report with per-size tables and interpretation |
| [`compare_dragons.py`](compare_dragons.py) | Reproducible report generator — parses the JSON files and emits the report |
| `dragon-*.json` | Raw benchmark JSON from the Cosmic Dragon (`main` branch) |
| `dead-*.json` | Raw benchmark JSON from the Dead Dragon (`dead-dragon` branch) |

Three screen sizes were tested, each with a 5-second wet-I/O benchmark
(`--benchmark --json --screen-size WxH --bench-duration 5 --bench-io`):

- `80x24` — common terminal
- `200x60` — large terminal
- `400x100` — stress

## Headline result

| Size | Cells/Frame | Cosmic Dragon FPS | Dead Dragon FPS | Dragon Advantage |
|---|---:|---:|---:|---:|
| 80×24 | 1,920 | 60,943 | 39,797 | **1.5× faster** |
| 200×60 | 12,000 | 18,404 | 8,119 | **2.3× faster** |
| 400×100 | 40,000 | 8,103 | 2,717 | **3.0× faster** |

The Cosmic Dragon renders only the cells that changed since the previous
frame (typically 3–9% of the grid). The Dead Dragon re-sends every cell
on every frame. The gap widens as screen size grows, because the Dead
Dragon's per-frame cost is O(W×H) while the Cosmic Dragon's is
O(active_rain_columns).

See [`comparison-report.md`](comparison-report.md) for full per-size
metrics, including render time, I/O time, ANSI bytes written, write()
syscalls, and dirty cell counts.

## Reproducing

```bash
# Build both binaries
git checkout main && cargo build --release
cp target/release/cosmostrix /tmp/cosmostrix-dragon

git checkout dead-dragon && cargo build --release
cp target/release/cosmostrix /tmp/cosmostrix-dead

# Run each benchmark (back-to-back, same host)
for size in 80x24 200x60 400x100; do
    /tmp/cosmostrix-dragon --benchmark --json --screen-size $size --bench-duration 5 --bench-io \
        > dragon-$size-wet.json
    /tmp/cosmostrix-dead --benchmark --json --screen-size $size --bench-duration 5 --bench-io \
        > dead-$size-wet.json
done

# Regenerate the report
python3 compare_dragons.py > comparison-report.md
```

## Verdict

The Dead Dragon branch is the perfect control subject: same codebase,
same simulation, same visual output — only the rendering method differs.
The 1.5×–3.0× FPS gap, the 3.1×–3.7× ANSI byte savings, and the
11.7×–30.3× dirty-cell reduction are the undeniable proof that the
Cosmic Dragon's diff-based engine is a real, measurable, world-class
innovation — not marketing fluff.

The Dead Dragon branch will be preserved indefinitely as a reference
control. Future engine improvements can re-run this A/B to quantify
their impact.
