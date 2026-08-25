#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# ─────────────────────────────────────────────────────────────────────────────
# cosmostrix Visual Preset Switcher — Preset Battle Round 2
#
# Switches the 17-parameter visual preset in src/central_control_rains/mod.rs
# between the reigning champion (Cinema Noir) and the round-2 challenger
# presets, so each can be A/B tested on a real terminal before the owner
# declares a new champion.
#
# USAGE:
#   ./scripts/apply-visual-preset.sh <preset>   # apply a preset
#   ./scripts/apply-visual-preset.sh show       # detect the active preset
#   ./scripts/apply-visual-preset.sh list       # list available presets
#
# After applying, rebuild for the change to take effect:
#   cargo build --release
#
# The script patches ONLY the 17 battle parameters (the preset identity
# package). The Option F inherited values (speed / length / density /
# selfbloom / monolith layers) are identical in every preset and are never
# touched.
#
# Preset definitions and design rationale:
#   docs/research/PRESET_BATTLE_2.md
# Current visual identity (single source of truth):
#   docs/VISUAL_IDENTITY.md
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${REPO_ROOT}/src/central_control_rains/mod.rs"

if [ ! -f "$TARGET" ]; then
	echo "ERROR: $TARGET not found" >&2
	exit 1
fi

python3 - "$TARGET" "$@" <<'PYTHON_SCRIPT'
import re
import sys
from pathlib import Path

# ── Preset table (17 battle parameters per preset) ─────────────────────────
# Values are formatted exactly as they appear in the source file.

PRESETS = {
    # Reigning champion (battle 1, 2026-08-17) — the current shipped default.
    "cinema-noir": {
        "EDGE_FADE_TOP_MIN": "0.45",
        "EDGE_FADE_BOTTOM_MIN": "0.65",
        "EDGE_FADE_BOTTOM_ROWS": "10",
        "EDGE_FADE_BOTTOM_LIP": "0.80",
        "VIGNETTE_INTENSITY": "0.20",
        "VIGNETTE_INNER_RADIUS": "0.7",
        "CRT_VIGNETTE_EDGE_FACTOR": "0.85",
        "RAIN_SHADOW_PCT": "0.15",
        "RAIN_SHADOW_FLOOR": "0.55",
        "PARALLAX_BRIGHTNESS_MULT": "[0.52, 0.80, 1.10]",
        "PARALLAX_SATURATION_MULT": "[0.50, 0.84, 1.12]",
        "PARALLAX_HEAD_BLOOM_MULT": "[0.48, 0.74, 1.30]",
        "PARALLAX_CONTRAST_REDUCTION": "[0.50, 0.18, 0.0]",
        "PHOSPHOR_DECAY_RATE": "5.0",
        "PHOSPHOR_LAYER_DECAY_MULT": "[2.0, 1.2, 0.6]",
        "PHOSPHOR_BOTTOM_DECAY_MULT": "2.0",
        "HEAD_BLOOM_INTENSITY": "0.40",
    },
    # Challenger 1 — endurance refinement of the noir narrative.
    "deep-focus": {
        "EDGE_FADE_TOP_MIN": "0.48",
        "EDGE_FADE_BOTTOM_MIN": "0.68",
        "EDGE_FADE_BOTTOM_ROWS": "12",
        "EDGE_FADE_BOTTOM_LIP": "0.82",
        "VIGNETTE_INTENSITY": "0.14",
        "VIGNETTE_INNER_RADIUS": "0.75",
        "CRT_VIGNETTE_EDGE_FACTOR": "0.87",
        "RAIN_SHADOW_PCT": "0.13",
        "RAIN_SHADOW_FLOOR": "0.58",
        "PARALLAX_BRIGHTNESS_MULT": "[0.56, 0.82, 1.08]",
        "PARALLAX_SATURATION_MULT": "[0.52, 0.84, 1.10]",
        "PARALLAX_HEAD_BLOOM_MULT": "[0.48, 0.74, 1.24]",
        "PARALLAX_CONTRAST_REDUCTION": "[0.50, 0.18, 0.0]",
        "PHOSPHOR_DECAY_RATE": "5.5",
        "PHOSPHOR_LAYER_DECAY_MULT": "[1.9, 1.15, 0.65]",
        "PHOSPHOR_BOTTOM_DECAY_MULT": "1.8",
        "HEAD_BLOOM_INTENSITY": "0.36",
    },
    # Challenger 2 — 35mm film-stock authenticity.
    "celluloid": {
        "EDGE_FADE_TOP_MIN": "0.42",
        "EDGE_FADE_BOTTOM_MIN": "0.62",
        "EDGE_FADE_BOTTOM_ROWS": "14",
        "EDGE_FADE_BOTTOM_LIP": "0.78",
        "VIGNETTE_INTENSITY": "0.24",
        "VIGNETTE_INNER_RADIUS": "0.66",
        "CRT_VIGNETTE_EDGE_FACTOR": "0.83",
        "RAIN_SHADOW_PCT": "0.17",
        "RAIN_SHADOW_FLOOR": "0.52",
        "PARALLAX_BRIGHTNESS_MULT": "[0.46, 0.78, 1.06]",
        "PARALLAX_SATURATION_MULT": "[0.44, 0.80, 1.06]",
        "PARALLAX_HEAD_BLOOM_MULT": "[0.44, 0.70, 1.22]",
        "PARALLAX_CONTRAST_REDUCTION": "[0.58, 0.22, 0.04]",
        "PHOSPHOR_DECAY_RATE": "4.2",
        "PHOSPHOR_LAYER_DECAY_MULT": "[2.2, 1.3, 0.55]",
        "PHOSPHOR_BOTTOM_DECAY_MULT": "2.4",
        "HEAD_BLOOM_INTENSITY": "0.34",
    },
    # Challenger 3 — luminous comfort for bright rooms and dark-adverse panels.
    "late-broadcast": {
        "EDGE_FADE_TOP_MIN": "0.55",
        "EDGE_FADE_BOTTOM_MIN": "0.72",
        "EDGE_FADE_BOTTOM_ROWS": "8",
        "EDGE_FADE_BOTTOM_LIP": "0.84",
        "VIGNETTE_INTENSITY": "0.10",
        "VIGNETTE_INNER_RADIUS": "0.8",
        "CRT_VIGNETTE_EDGE_FACTOR": "0.91",
        "RAIN_SHADOW_PCT": "0.10",
        "RAIN_SHADOW_FLOOR": "0.65",
        "PARALLAX_BRIGHTNESS_MULT": "[0.60, 0.84, 1.12]",
        "PARALLAX_SATURATION_MULT": "[0.56, 0.86, 1.14]",
        "PARALLAX_HEAD_BLOOM_MULT": "[0.52, 0.76, 1.28]",
        "PARALLAX_CONTRAST_REDUCTION": "[0.42, 0.14, 0.0]",
        "PHOSPHOR_DECAY_RATE": "6.0",
        "PHOSPHOR_LAYER_DECAY_MULT": "[1.8, 1.1, 0.55]",
        "PHOSPHOR_BOTTOM_DECAY_MULT": "1.7",
        "HEAD_BLOOM_INTENSITY": "0.42",
    },
}

PRESET_LABELS = {
    "cinema-noir": "Cinema Noir (reigning champion)",
    "deep-focus": "Deep Focus (challenger 1 - endurance)",
    "celluloid": "Celluloid (challenger 2 - film stock)",
    "late-broadcast": "Late Broadcast (challenger 3 - luminous)",
}


def read_values(text):
    """Extract the current value of every battle constant from the source."""
    values = {}
    for name in next(iter(PRESETS.values())):
        pattern = re.compile(
            r"^pub\(crate\) const " + name + r": [^=]+ = ([^;]+);$", re.MULTILINE
        )
        m = pattern.search(text)
        if m is None:
            sys.exit(f"ERROR: constant {name} not found in source — aborting")
        values[name] = m.group(1).strip()
    return values


def main():
    target = Path(sys.argv[1])
    cmd = sys.argv[2] if len(sys.argv) > 2 else "show"

    if cmd == "list":
        for key, label in PRESET_LABELS.items():
            print(f"  {key:16} {label}")
        return

    text = target.read_text()
    current = read_values(text)

    if cmd == "show":
        matches = [
            name
            for name, table in PRESETS.items()
            if all(current[k] == v for k, v in table.items())
        ]
        if len(matches) == 1:
            print(f"Active preset: {matches[0]} — {PRESET_LABELS[matches[0]]}")
        elif not matches:
            print("Active preset: CUSTOM (no preset table matches all 17 values)")
        else:
            print(f"Active preset: ambiguous ({', '.join(matches)})")
        print()
        print("Current battle values:")
        for k, v in current.items():
            print(f"  {k:32} = {v}")
        return

    if cmd not in PRESETS:
        sys.exit(
            f"ERROR: unknown preset '{cmd}'. Available: "
            + ", ".join(PRESETS) + " (or 'show' / 'list')"
        )

    table = PRESETS[cmd]
    changed = []
    for name, value in table.items():
        if current[name] == value:
            continue
        pattern = re.compile(
            r"^(pub\(crate\) const " + name + r": [^=]+ = )[^;]+;$", re.MULTILINE
        )
        matches = pattern.findall(text)
        if len(matches) != 1:
            sys.exit(
                f"ERROR: expected exactly 1 definition of {name}, found "
                f"{len(matches)} — aborting without writing"
            )
        text = pattern.sub(lambda m: m.group(1) + value + ";", text, count=1)
        changed.append((name, current[name], value))

    if not changed:
        print(f"Preset '{cmd}' is already active — nothing to do.")
        return

    target.write_text(text)
    print(f"Applied preset: {cmd} — {PRESET_LABELS[cmd]}")
    print()
    print("Changed constants:")
    for name, old, new in changed:
        print(f"  {name:32} {old}  ->  {new}")
    print()
    print("Rebuild to see it:  cargo build --release")
    print("Revert to champion: ./scripts/apply-visual-preset.sh cinema-noir")


main()


PYTHON_SCRIPT
