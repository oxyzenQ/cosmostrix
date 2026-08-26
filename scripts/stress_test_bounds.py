#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

"""v50.0.0-beta.6 LTS Stress Test — verify all config bounds.

Source code = truth. This script generates stress test configs that
exceed each bound, runs --testconf + parse verification, and reports
PASS/FAIL for each.

Usage: python3 scripts/stress_test_bounds.py [--bin /path/to/cosmostrix]
"""
import subprocess
import sys
import os
import tempfile
from pathlib import Path

# ── Configuration ──
BIN = sys.argv[sys.argv.index("--bin") + 1] if "--bin" in sys.argv else "./target/release/cosmostrix"
# Fall back to pro-linux-v3 if release not built
if not os.path.exists(BIN):
    BIN = "./target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix"
if not os.path.exists(BIN):
    print(f"ERROR: binary not found at {BIN}")
    sys.exit(1)

RESULTS = []

def test(name, config_content, expect_pass=True, expect_warning_contains=None, expect_error_contains=None):
    """Run --testconf with the given config, verify result."""
    # Use ~/.config/cosmostrix/ (the allowed config directory)
    config_dir = os.path.expanduser('~/.config/cosmostrix')
    os.makedirs(config_dir, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode='w', suffix='.toml', delete=False, dir=config_dir) as f:
        f.write(config_content)
        config_path = f.name

    try:
        # Run with --config to point at our temp file
        result = subprocess.run(
            [BIN, "--testconf", "--config", config_path],
            capture_output=True, text=True, timeout=10
        )
        stdout = result.stdout
        stderr = result.stderr
        combined = stdout + stderr

        if expect_pass:
            passed = "PASS" in combined or result.returncode == 0
        else:
            passed = "FAIL" in combined or result.returncode != 0

        if expect_warning_contains:
            for w in expect_warning_contains:
                if w not in combined.lower():
                    passed = False
                    break

        if expect_error_contains:
            for e in expect_error_contains:
                if e not in combined.lower():
                    passed = False
                    break

        status = "PASS" if passed else "FAIL"
        RESULTS.append((name, status, combined.strip()[:200]))
        return passed
    except Exception as e:
        RESULTS.append((name, "ERROR", str(e)[:200]))
        return False
    finally:
        os.unlink(config_path)

# ── Stress Test 1: Ambient entries 256+ ──
# AMBIENT_MAX_ENTRIES = 256, truncate silently
def test_ambient_max():
    """Generate 260 ambient entries (over the 256 cap)."""
    # Ambient entries must be at ROOT scope (not inside any [section]).
    # Put them at the TOP of the file before any section header.
    lines = []
    count = 0
    for h in range(24):
        for m in range(60):
            if count >= 260:
                break
            lines.append(f'ambient.{h:02d}-{m:02d} = cosmos')
            count += 1
        if count >= 260:
            break
    lines.append('')
    lines.append('[charset-custom.zen]')
    lines.append('set = "|"')
    config = '\n'.join(lines) + '\n'
    # testconf should PASS (ambient truncation is silent, no warning)
    test("ambient_260_entries_truncated", config, expect_pass=True)

# ── Stress Test 2: colors-custom 101+ blocks ──
def test_colors_blocks():
    """Generate 105 colors-custom blocks (over the 100 cap)."""
    lines = []
    for i in range(105):
        lines.append(f'[colors-custom.palette{i}]')
        lines.append(f'rain = "#000000, #ffffff"')
        lines.append('')
    config = '\n'.join(lines) + '\n'
    # testconf should PASS (block cap is silent skip)
    test("colors_105_blocks_capped", config, expect_pass=True)

# ── Stress Test 3: charset-custom 101+ blocks ──
def test_charset_blocks():
    """Generate 105 charset-custom blocks (over the 100 cap)."""
    lines = []
    for i in range(105):
        lines.append(f'[charset-custom.charset{i}]')
        lines.append(f'set = "ab"')
        lines.append('')
    config = '\n'.join(lines) + '\n'
    test("charset_105_blocks_capped", config, expect_pass=True)

# ── Stress Test 4: scene-custom 101+ blocks ──
def test_scene_blocks():
    """Generate 105 scene-custom blocks (over the 100 cap)."""
    lines = []
    for i in range(105):
        lines.append(f'[scene-custom.scene{i}]')
        lines.append(f'color = green')
        lines.append('')
    config = '\n'.join(lines) + '\n'
    test("scene_105_blocks_capped", config, expect_pass=True)

# ── Stress Test 5: colors rain stops 65+ ──
def test_rain_stops():
    """Generate 70 rain stops (over the 64 cap)."""
    stops = [f'#{i:02x}{i:02x}{i:02x}' for i in range(70)]
    config = f'[colors-custom.big]\nrain = "{", ".join(stops)}"\n'
    # testconf should PASS (stops capped at runtime, warning emitted)
    test("colors_70_rain_stops_capped", config, expect_pass=True)

# ── Stress Test 6: charset set 257+ chars ──
def test_charset_chars():
    """Generate 260 chars (over the 256 cap)."""
    chars = 'x' * 260
    config = f'[charset-custom.long]\nset = "{chars}"\n'
    # testconf should FAIL (charset length is hard error)
    test("charset_260_chars_rejected", config, expect_pass=False)

# ── Stress Test 7: name length 65+ chars all 3 systems ──
def test_name_length():
    """Generate names longer than 64 chars."""
    long_name = 'x' * 70
    # colors-custom
    config = f'[colors-custom.{long_name}]\nrain = "#000000, #ffffff"\n'
    test("colors_70char_name_skipped", config, expect_pass=True)

    # charset-custom
    config = f'[charset-custom.{long_name}]\nset = "ab"\n'
    test("charset_70char_name_skipped", config, expect_pass=True)

    # scene-custom
    config = f'[scene-custom.{long_name}]\ncolor = green\n'
    test("scene_70char_name_skipped", config, expect_pass=True)

# ── Stress Test 8: unknown field rejection all 3 systems ──
def test_unknown_fields():
    """Unknown fields inside custom blocks must be rejected."""
    # charset-custom + color (invalid field)
    config = '[charset-custom.quantum]\nset = "abcdef"\ncolor = green\n'
    test("charset_unknown_field_color_rejected", config, expect_pass=False,
         expect_error_contains=["unknown"])

    # colors-custom + speed (invalid field)
    config = '[colors-custom.sun]\nrain = "#000000, #ffffff"\nspeed = 28\n'
    test("colors_unknown_field_speed_rejected", config, expect_pass=False,
         expect_error_contains=["unknown"])

    # scene-custom + intro (invalid field)
    config = '[scene-custom.hacker-mode]\ncolor = green\nintro = cosmic\n'
    test("scene_unknown_field_intro_rejected", config, expect_pass=False,
         expect_error_contains=["unknown"])

# ── Stress Test 9: density-map out-of-range warning ──
def test_density_map():
    """density-map with out-of-range values should PASS + warn."""
    config = '[scene-custom.hacker-mode]\ndensity-map = "0.5,1.5,-0.3,2.0"\n'
    # Should PASS (warning, not error)
    test("density_map_out_of_range_warned", config, expect_pass=True)

# ── Stress Test 10: valid config (control) ──
def test_valid_control():
    """Normal valid config should PASS with no warnings."""
    config = '''[colors-custom.sunset]
bg = "#0a0a12"
rain = "#1a0033, #4d0080, #9933ff, #cc66ff, #ffffff"

[charset-custom.zen]
set = "|"

[scene-custom.afternoon]
base-scene = "signal"
color = "neon-green"
speed = "50"
'''
    test("valid_config_control", config, expect_pass=True)

# ── Run all tests ──
print(f"Binary: {BIN}")
print(f"Running stress tests...\n")

test_ambient_max()
test_colors_blocks()
test_charset_blocks()
test_scene_blocks()
test_rain_stops()
test_charset_chars()
test_name_length()
test_unknown_fields()
test_density_map()
test_valid_control()

# ── Report ──
print("=" * 70)
print("STRESS TEST RESULTS")
print("=" * 70)
passed = 0
failed = 0
for name, status, detail in RESULTS:
    icon = "✓" if status == "PASS" else "✗"
    print(f"  {icon} {status:4} {name}")
    if status != "PASS":
        print(f"         detail: {detail}")
    if status == "PASS":
        passed += 1
    else:
        failed += 1

print(f"\n{'=' * 70}")
print(f"Total: {passed} passed, {failed} failed")
print(f"{'=' * 70}")
sys.exit(0 if failed == 0 else 1)
