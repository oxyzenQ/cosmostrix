<!-- SPDX-License-Identifier: GPL-3.0-only -->

# simple tools I usually use

The owner's asset-toolkit note: the exact commands that produced the
`assets/` imagery the README renders. Recorded here so the next asset
refresh starts from the working recipe instead of rediscovering the
flags.

## animated webp for the README demo loop

```bash
ffmpeg -i cosmostrix-video.webm -c:v libwebp_anim -vf "fps=20,scale=960:-1:flags=lanczos" -quality 80 -compression_level 6 -loop 0 -an cosmostrix-video.webp
```

## rounded corners for the README display images

```bash
magick input.png \( -size 1254x1254 xc:none -fill white -draw 'roundrectangle 0,0 1253,1253 68,68' \) -alpha off -compose CopyOpacity -composite output.png
```

## terminal window size for demo captures

```sh
alacritty -o "window.dimensions.columns=120" -o "window.dimensions.lines=40"
```

120x40 is the capture window the README's terminal shots assume —
wide enough for the full HUD row, short enough to stay legible in a
screenshot.

## blake3 (b3sum) signature checksums for the asset pngs

night-dragon-maintenance-X1 (2026-11-01): every png under `assets/`
(the two logo plates, the 4-scene capture, and the three donation qr
codes) carries a recorded BLAKE3 digest, so a future swap, duplicate,
or re-encoded fake of the logo is detectable against the committed
bytes. The digests were produced with `b3sum 1.8.7` and cross-verified
against an independent BLAKE3 implementation before landing.

Tool: `cargo install b3sum` (the BLAKE3 team's canonical CLI).

Regenerate and compare against the table below — any mismatch means
the file on disk no longer matches the signed asset:

```bash
cd assets
b3sum $(find . -name '*.png' | sort)
```

| file | bytes | blake3 |
|------|-------|--------|
| cosmostrix-4-scene.png | 184815 | `65cd77593207b365f6c8f3fb00b70ffac77803918356b49f7873eec2fa6d93ed` |
| cosmostrix-original-logo-2026-old.png | 1255879 | `98b3862b7b7e0ed4dd286cd8dd6d63acec4ff1589c660ebf334a5722d3f135cd` |
| cosmostrix-original-logo-2026.png | 1288029 | `dd99531e39307c0ea0766ab28651bb64bb7f1f9ecbc18bb8746cc38c4795e4ab` |
| qr/bitcoin.png | 575 | `09ec09fcdc4815e23373cb266bb120aa4e1af140fb9d598e117d05478a791b47` |
| qr/ethereum.png | 494 | `f0f3bf92d1730fe304f57f591ba1ed036449bb13731624e2e5b83c5c9896750b` |
| qr/solana.png | 595 | `bca56b0a454daf06ff2d57fe401719553e28733e73479de2652a0740a9e87d1a` |

The animated webp (`cosmostrix-video.webp`) is out of the png table's
scope; the same `b3sum` command covers it if the owner ever wants it
pinned too. When an asset legitimately changes, refresh the matching
row in the same commit that replaces the png.

<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying on
  any specific number (test count, LOC, FPS, ms timeout), file path,
  function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
