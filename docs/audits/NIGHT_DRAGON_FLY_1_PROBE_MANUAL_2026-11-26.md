<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

# night-dragon-fly-1 — UX CLI end-to-end probe manual (2026-11-26)

Purpose: one copy-paste battery that exercises EVERY surface advertised in
`cosmostrix -h/--help`, so the captured output can be audited for
print-json / UX / CLI / wording / suggestion problems before LTS landing.

Run order matters for the baseline roundtrip (section 4). Sections 1-6
and 8 are safe, scriptable, non-interactive. Section 7 is interactive
and must be run by hand in a real terminal. Nothing here launches a
runaway process; every interactive command is either not scriptable by
nature or wrapped so it self-exits.

Conventions used below:

```bash
# helper: prints the command banner, merges stdout+stderr, prints exit code
probe() { printf '\n===== $ %s =====\n' "$*"; "$@" 2>&1; printf '----- exit=%d -----\n' "$?"; }
```

Recommended capture (send the whole file back):

```bash
{ script -qec '
  probe() { printf "\n===== \$ %s =====\n" "$*"; "$@" 2>&1; printf -- "----- exit=%d -----\n" "$?"; }
  # paste the probe lines from sections 1-6 and 8 here
' /dev/null; } > cosmostrix_ux_probe_output.txt 2>&1
```

Section 0 — context (always first)

```bash
cosmostrix -V
cosmostrix --version
cosmostrix -h | head -30
cosmostrix --help
cosmostrix sig; echo "sig exit=$?"
```

Section 1 — discovery surfaces

```bash
probe cosmostrix --list-colors
probe cosmostrix --list-charsets
probe cosmostrix --list-scenes
probe cosmostrix --list-scenes --json
probe cosmostrix --show-scene matrix
probe cosmostrix --show-scene cosmic-dragon
probe cosmostrix --show-scene does_not_exist
probe cosmostrix --color definitely_not_a_theme
probe cosmostrix --charset no_such_charset
probe cosmostrix --scene no_such_scene
probe cosmostrix --scene-custom no_such_custom_scene
```

Section 2 — config surfaces (uses only the whitelisted dir; adjust $HOME)

```bash
probe cosmostrix --config-path
probe cosmostrix --dump-config
cosmostrix --dump-config | head -20
probe cosmostrix --dump-config "$HOME/.config/cosmostrix/probe_dump.toml"
probe cosmostrix --dump-config "$HOME/.config/cosmostrix/probe_dump.toml"
probe cosmostrix --dump-config "$HOME/.config/cosmostrix/probe_dump.toml" --force
probe cosmostrix --dump-config "$HOME/.config/cosmostrix/probe_dump.conf"
probe cosmostrix --dump-config /tmp/probe_dump.toml
cosmostrix --dump-config > /tmp/blocked_redirect.toml; echo "redirect exit=$?"
probe cosmostrix --config /tmp/probe_dump.toml --testconf
probe cosmostrix --config "$HOME/.config/cosmostrix/probe_dump.toml" --testconf
probe cosmostrix --testconf
probe cosmostrix --config "$HOME/.config/cosmostrix/probe_dump.toml" --doctor
probe cosmostrix --config /tmp/nowhere.toml
```

Section 3 — diagnostics surfaces

```bash
probe cosmostrix --doctor
cosmostrix --doctor | grep -i -E "renderer|color" ; echo "grep exit=$?"
probe cosmostrix --docs
cosmostrix --docs | grep -c $'\033\[' ; echo "docs ANSI-sequence count above (0 expected)"
probe cosmostrix --check-update
cosmostrix --docs | less
```

Section 4 — benchmark + print-json surfaces (the A/B evidence core)

```bash
probe cosmostrix --benchmark --bench-duration 2 --screen-size 80x24
cosmostrix --benchmark --bench-duration 2 --screen-size 80x24 --json > /tmp/ctx_report.json; echo "json exit=$?"
wc -l /tmp/ctx_report.json
python3 -c "import json;d=json.load(open('/tmp/ctx_report.json'));print(sorted(d.keys()))"
# NOTE: every baseline file lives under the whitelisted dir; a /tmp path is
# rejected by the safepath whitelist before the JSON is ever read.
cosmostrix --benchmark --bench-duration 2 --screen-size 80x24 --save-baseline "$HOME/.config/cosmostrix/ctx_baseline.json" > /dev/null 2>&1; echo "save exit=$?"
probe cosmostrix --benchmark --bench-duration 2 --screen-size 80x24 --compare-baseline "$HOME/.config/cosmostrix/ctx_baseline.json"
probe cosmostrix --benchmark --bench-duration 2 --screen-size 80x24 --save-baseline "$HOME/.config/cosmostrix/ctx_baseline.json"
probe cosmostrix --benchmark --save-baseline base.json
echo '{"broken":' > "$HOME/.config/cosmostrix/ctx_bad.json"
probe cosmostrix --benchmark --bench-duration 2 --screen-size 80x24 --compare-baseline "$HOME/.config/cosmostrix/ctx_bad.json"
probe cosmostrix --benchmark --bench-duration 2 --screen-size 80x24 --compare-baseline "$HOME/.config/cosmostrix/ghost.json"
echo '{"avg_fps":"99999.0","peak_fps":"99999.0"}' > "$HOME/.config/cosmostrix/ctx_hi.json"
probe cosmostrix --benchmark --bench-duration 2 --screen-size 80x24 --compare-baseline "$HOME/.config/cosmostrix/ctx_hi.json"
probe cosmostrix --benchmark --bench-duration 2 --screen-size 80x24 --bench-io --bench-scene production-draw
probe cosmostrix --benchmark --bench-duration 2 --screen-size 80x24 --bench-io --bench-scene lean
probe cosmostrix --benchmark --bench-io --bench-scene leanax
probe cosmostrix --benchmark --bench-duration 2 --bench-scene production-draw
probe cosmostrix --bench-all --bench-duration 1
cosmostrix --bench-all --bench-duration 1 --json > /tmp/ctx_scale.json; echo "bench-all json exit=$?"
head -c 400 /tmp/ctx_scale.json; echo
```

Section 5 — invalid-input / wording / suggestion probes (all must exit 2 with a specific hint)

```bash
probe cosmostrix --colour rainbow
probe cosmostrix --list-colours
probe cosmostrix --LIS
probe cosmostrix --disable-effects
probe cosmostrix --mouse
probe cosmostrix --uniform
probe cosmostrix --noglitch
probe cosmostrix --info
probe cosmostrix --message-border
probe cosmostrix --glitchpct 5
probe cosmostrix --sig
probe cosmostrix -G 5
probe cosmostrix -r 5
probe cosmostrix -i
probe cosmostrix -a
probe cosmostrix -mfss
probe cosmostrix --msg-fill-styl fade
probe cosmostrix --msg-fill-style bogus_style
probe cosmostrix -mfs bogus_style
probe cosmostrix --speed 12.5
probe cosmostrix -S 12.5
probe cosmostrix -S12.5
probe cosmostrix --fps 0
probe cosmostrix --fps 300
probe cosmostrix --fps 1e2
probe cosmostrix --density 0
probe cosmostrix --density 9
probe cosmostrix --color-mode 9
probe cosmostrix --color-mode 8
probe cosmostrix --bold 3
probe cosmostrix --shading-mode 5
probe cosmostrix --glitch-ms 400,300
probe cosmostrix --glitch-ms 0,6000
probe cosmostrix --linger-ms 5,70000
probe cosmostrix --duration -5
probe cosmostrix --duration 1e9
probe cosmostrix --duration NaN
probe cosmostrix --duration 1h30
probe cosmostrix --duration 0.5d
probe cosmostrix --bench-duration 0
probe cosmostrix --bench-duration 0.5
probe cosmostrix --bench-duration 0.5d
probe cosmostrix --bench-duration 25h
probe cosmostrix --bench-duration 86400
probe cosmostrix --bench-duration 1w
probe cosmostrix --screen-size 0x0
probe cosmostrix --screen-size 120x
probe cosmostrix --screen-size abc
probe cosmostrix --screen-size 5000x3000
probe cosmostrix --bench-frames 0
probe cosmostrix --crystal-dragon 10
probe cosmostrix --crystal-dragon yes
probe cosmostrix --crystal-dragon-secs 2m
probe cosmostrix --intro warp
probe cosmostrix --bench-scene leanax
probe cosmostrix --json
probe cosmostrix --bench-cosmetics
probe cosmostrix --bench-frames 100 --benchmark --bench-duration 2
probe cosmostrix --benchmark --bench-frames 100 --bench-duration 2 --screen-size 80x24
probe cosmostrix --duration 10 --benchmark --bench-duration 2 --screen-size 80x24
probe cosmostrix sig extra_token
probe cosmostrix -mb
probe cosmostrix -m ""
probe cosmostrix -m -mb hello
probe cosmostrix -mb --fps 30
probe cosmostrix --monolith-size huge
probe cosmostrix --color-bg mid-gray
probe cosmostrix --crystal-dragon
probe cosmostrix --power-dragon
probe cosmostrix --msg-mode
probe cosmostrix --async-mode
```

Section 6 — color/env gating surfaces (help colorization contract)

```bash
cosmostrix --help | head -5
NO_COLOR=1 cosmostrix --help | head -5
CLICOLOR=0 cosmostrix --help | head -5
CLICOLOR_FORCE=1 cosmostrix --help | head -5
cosmostrix --help | grep -c $'\033\[' ; echo "piped help ANSI count above (0 expected)"
CLICOLOR_FORCE=1 cosmostrix --help | grep -c $'\033\[' ; echo "forced piped help ANSI count above (>0 expected)"
NO_COLOR=1 cosmostrix --list-colors | head -5
```

Section 7 — interactive surfaces (run BY HAND in a real terminal, one at a time; q quits; note key responses)

```bash
cosmostrix --duration 12
cosmostrix --screensaver --duration 12
cosmostrix --intro cosmic --duration 12
cosmostrix --intro none --scene matrix --duration 12
cosmostrix --scene cosmic-dragon --duration 12
cosmostrix -m "hello world" --duration 12
cosmostrix -mb "wake up, neo" -mfs engrave --duration 12
cosmostrix -mb "wake up, neo" -mfs radar --duration 12
cosmostrix -mb "wake up, neo" -mfs glitch --duration 12
cosmostrix --msg-mode false --duration 12
cosmostrix --no-effects --duration 12
cosmostrix --crystal-dragon true --crystal-dragon-secs 15s --duration 20
cosmostrix --power-dragon false --duration 12
cosmostrix --verbose --duration 12
cosmostrix --perf-stats --duration 12
cosmostrix -v --duration 12
cosmostrix --screen-size 5000x3000 --duration 12
cosmostrix --color-tune sat=1.5,bright=0.9 --duration 12
cosmostrix --async-mode false --duration 12
cosmostrix --monolith-size large --scene monolith --duration 12
```

During any interactive run, exercise once and note behavior: `c` `C`
`s` `S` `x` `X` `[` `]` Up Down `r` `i` `p` (then `p` again) `q`;
mouse hover glow + click shockwave; `q` mid-intro skip.

Section 8 — JSON deep checks (machine contract)

```bash
python3 - <<'EOF'
import json
d = json.load(open('/tmp/ctx_report.json'))
print("single line ok:", len(open('/tmp/ctx_report.json').read().strip().splitlines()) == 1)
print("top-level sections:", len(d.keys()))
for k in ("status","system","renderer","config","environment","performance","memory","cpu","resource","component_timing","cell_efficiency","drift","throughput","timing","terminal_io","energy","microarchitecture","allocator","visual_objective"):
    print(f"{k}: {'present' if k in d else 'MISSING'}")
print("nan check:", "null" not in open('/tmp/ctx_report.json').read()[:0] and any(v is None for s in d.values() if isinstance(s, dict) for v in s.values()))
EOF
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
