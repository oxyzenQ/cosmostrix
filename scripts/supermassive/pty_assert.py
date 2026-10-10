#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Reads bytes written by
#   pty_smoke.py; no pty of its own.
"""Byte assertions over pty smoke captures (night-improve-7).

Reads a tty byte log written by pty_smoke.py and checks named control
sequences against it. Sequences are referenced by NAME because the raw
bytes contain ESC and CSI fragments that must never travel through
shell arguments or YAML scalars; the name-to-bytes table below is the
single place where the bytes are spelled out.

The color-tier names mirror the capability-aware palette contract in
src/output/mod.rs (truecolor / 256-color / 16-color / Mono):

- alt-enter / alt-leave   alternate screen enter and leave
- mouse-off               mouse tracking disable (single canonical
                          token, order-independent: the restore stack
                          emits the tracking disables in reverse order
                          on optimized builds)
- paste-off               bracketed paste disable
- focus-off               focus-events disable
- sgr-truecolor           a 38;2 / 48;2 direct-color SGR
- sgr-256                 a 38;5 / 48;5 indexed-color SGR
- sgr-16                  a basic 30-37 / 90-97 foreground SGR
- sgr-any                 any SGR introduction at all

Sentinel contract: one MASS-RESULT row per checked sequence, ending in
PASS or FAIL; exits 3 when any check fails, 2 on usage errors.
"""

from __future__ import annotations

import argparse
import sys

ESC = b"\x1b"

SEQ_TABLE: dict[str, bytes] = {
    "alt-enter": ESC + b"[?1049h",
    "alt-leave": ESC + b"[?1049l",
    "mouse-off": ESC + b"[?1000l",
    "paste-off": ESC + b"[?2004l",
    "focus-off": ESC + b"[?1004l",
    "sgr-truecolor": ESC + b"[38;2",
    "sgr-256": ESC + b"[38;5",
    "sgr-16": ESC + b"[3",  # 30-37 foreground tier (matches 31, 32, ...)
    "sgr-any": ESC + b"[",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check named terminal sequences in a pty capture log."
    )
    parser.add_argument("--log", required=True, help="byte log written by pty_smoke.py")
    parser.add_argument(
        "--mass", required=True, help="file to append MASS-RESULT rows to"
    )
    parser.add_argument(
        "--label", required=True, help="leg name used in MASS-RESULT rows"
    )
    parser.add_argument(
        "--expect",
        action="append",
        default=[],
        dest="expect",
        metavar="NAME",
        help="sequence that MUST appear, repeatable (see SEQ_TABLE)",
    )
    parser.add_argument(
        "--forbid",
        action="append",
        default=[],
        dest="forbid",
        metavar="NAME",
        help="sequence that MUST NOT appear, repeatable (see SEQ_TABLE)",
    )
    return parser.parse_args()


def append_row(mass_path: str, label: str, text: str) -> None:
    with open(mass_path, "a", encoding="utf-8") as handle:
        handle.write(f"MASS-RESULT: {label} {text}\n")


def main() -> int:
    args = parse_args()
    unknown = [name for name in (*args.expect, *args.forbid) if name not in SEQ_TABLE]
    if unknown:
        print(f"pty_assert: unknown sequence names {unknown}", file=sys.stderr)
        return 2

    with open(args.log, "rb") as handle:
        data = handle.read()

    failed = 0
    for name in args.expect:
        if SEQ_TABLE[name] in data:
            append_row(args.mass, args.label, f"expect {name}: PASS")
        else:
            append_row(args.mass, args.label, f"expect {name}: FAIL")
            failed += 1
    for name in args.forbid:
        if SEQ_TABLE[name] in data:
            append_row(args.mass, args.label, f"forbid {name}: FAIL")
            failed += 1
        else:
            append_row(args.mass, args.label, f"forbid {name}: PASS")

    return 3 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
