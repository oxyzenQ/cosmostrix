<!-- SPDX-License-Identifier: GPL-3.0-only -->

# simple tools I usually use

The owner's asset-toolkit note: the exact commands that produced the
`assets/` imagery the README renders. Recorded here so the next asset
refresh starts from the working recipe instead of rediscovering the
flags.

### animated webp for the README demo loop

```bash
ffmpeg -i cosmostrix-video.webm -c:v libwebp_anim -vf "fps=20,scale=960:-1:flags=lanczos" -quality 80 -compression_level 6 -loop 0 -an cosmostrix-video.webp
```

### rounded corners for the README display images

```bash
magick input.png \( -size 1254x1254 xc:none -fill white -draw 'roundrectangle 0,0 1253,1253 68,68' \) -alpha off -compose CopyOpacity -composite output.png
```

### terminal window size for demo captures

```sh
alacritty -o "window.dimensions.columns=120" -o "window.dimensions.lines=40"
```

120x40 is the capture window the README's terminal shots assume —
wide enough for the full HUD row, short enough to stay legible in a
screenshot.

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
