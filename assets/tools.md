<!-- SPDX-License-Identifier: GPL-3.0-only -->

# simple tools I usually use

```bash
# for gif
gifski -o cosmostrix-video.gif --fps 60 --quality 100 cosmostrix-video.webm

# for webp
ffmpeg -i cosmostrix-video.webm -c:v libwebp_anim -vf "fps=20,scale=960:-1:flags=lanczos" -quality 80 -compression_level 6 -loop 0 -an cosmostrix-video.webp
```
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
