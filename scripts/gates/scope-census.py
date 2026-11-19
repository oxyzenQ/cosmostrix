#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
"""Scope census pin (night-frontier-Z6).

The README's never-be contract is a closed list of what cosmostrix
will NEVER be: a daemon or background service, a sound engine, a game,
a network service, a GUI application, a machine-learning runtime, a
plugin host, and a cloud or distributed system. A contract nobody
enforces is a press release, so this gate pins the two surfaces such
an identity change would have to touch:

  1. The direct dependency manifest (Cargo.toml). Every identity
     above has a dependency signature: audio backends (cpal, rodio,
     alsa), GUI toolkits (winit, wgpu, egui), embedded network stacks
     (tokio, hyper, reqwest), dynamic-library loaders (libloading),
     ML runtimes (candle, ndarray, tract). The manifest's direct
     dependency set is pinned exactly - the main table, the
     target-specific tables, and the dev table - so any addition or
     removal is a finding the owner must consciously re-land.
  2. The source tree (src/). A capability can also arrive without a
     new crate, through a direct use of the standard library's
     socket APIs or a new child-process site. The sweep is
     zero-tolerance for socket, embedded-network, audio, windowing,
     plugin-loader, and ML capability markers, and the
     process::Command census is pinned to the files that carry it
     today.

Counting rule (deterministic, mirrors unsafe-census.py): a line
counts when the pattern matches it AND the line's stripped form does
not start with `//` (plain and doc comments are prose, not surface).
Capability markers are word-boundary regexes, so prose like
"hyperbolic", "hyperthreading", "brightness pulse", or phosphor
"burn-in" never reads as the crate it merely sounds like.

Scope honesty: transitive dependencies are OUT of this gate's
contract. The pin is what cosmostrix itself declares and calls - the
project's own identity surface - not what every transitive crate is
technically capable of. The transitive graph is policed by
cargo-deny and cargo-audit, which own that layer. The single
sanctioned outbound network touch is the user-invoked update check,
which shells out to curl/wget instead of embedding an HTTP stack;
that delegation is exactly what keeps the manifest free of net crates.

Exit codes: 0 = census matches the pins; 1 = drift (a new or removed
dependency, a capability marker in src/, or a new process::Command
site); 2 = usage or tree fault.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

# The pinned direct dependency set, exactly as the manifest carries it
# at the night-frontier-Z6 SHA. Every entry is a local-capability
# crate: terminal input and raw-mode output (crossterm), argument
# parsing (clap), random sequences (rand), compact containers
# (bitvec, smallvec), glyph metrics (unicode-width), filesystem
# watching (notify, also redeclared per-target for backend
# features), signal handling (signal-hook, ctrlc), the libc binding
# layer, and hashing (sha2, blake3). loom and proptest are test-only
# dev dependencies and never ship in the binary. None of them speaks
# sockets, audio, windows, or models.
PINNED_DEPENDENCIES: frozenset[str] = frozenset(
    {
        "bitvec",
        "blake3",
        "clap",
        "ctrlc",
        "crossterm",
        "libc",
        "notify",
        "rand",
        "sha2",
        "signal-hook",
        "smallvec",
        "unicode-width",
    }
)
PINNED_DEV_DEPENDENCIES: frozenset[str] = frozenset(
    {
        "loom",
        "proptest",
    }
)

# Zero-tolerance capability markers. Each family is the dependency or
# API signature of one never-be identity; a single non-comment hit in
# src/ fails the gate. Families are named for the identity they would
# import, so a finding is readable as the contract clause it crosses.
CAPABILITY_MARKERS: dict[str, re.Pattern[str]] = {
    "socket API (the network-service identity)": re.compile(
        r"\bstd::net\b"
        r"|\bTcpListener\b"
        r"|\bTcpStream\b"
        r"|\bUdpSocket\b"
        r"|\bto_socket_addrs\b"
        r"|\bUnixListener\b"
        r"|\bUnixStream\b"
    ),
    "embedded network stack (the network-client identity)": re.compile(
        r"\btokio\b"
        r"|\basync_std\b"
        r"|\bhyper\b"
        r"|\breqwest\b"
        r"|\bureq\b"
        r"|\bactix\b"
        r"|\battohttpd\b"
        r"|\btiny_http\b"
        r"|\bsurf\b"
    ),
    "audio backend (the sound-engine identity)": re.compile(
        r"\bcpal\b"
        r"|\brodio\b"
        r"|\balsa\b"
        r"|\bpulseaudio\b"
        r"|\blibpulse\b"
        r"|\bsymphonia\b"
        r"|\bminimp3\b"
        r"|\bhound\b"
    ),
    "windowing or GUI toolkit (the GUI-application identity)": re.compile(
        r"\bwinit\b"
        r"|\bwgpu\b"
        r"|\bglium\b"
        r"|\btao\b"
        r"|\bsdl2\b"
        r"|\bsdl3\b"
        r"|\bsfml\b"
        r"|\begui\b"
        r"|\biced\b"
        r"|\bgtk\b"
        r"|\bfltk\b"
    ),
    "dynamic-library loader (the plugin-host identity)": re.compile(
        r"\blibloading\b"
        r"|\bdlopen\b"
    ),
    "machine-learning runtime (the ML identity)": re.compile(
        r"\bcandle\b"
        r"|\btch\b"
        r"|\btract\b"
        r"|\bndarray\b"
        r"|\bonnx\b"
        r"|\bort\b"
    ),
}

# The pinned process::Command census. The terminal-restore path and
# the user-invoked update check (which delegates to curl/wget on PATH
# - no embedded HTTP stack) are the only sanctioned child-process
# sites; a new one is a finding, because arbitrary child processes are
# the generic escape hatch every identity above would ride through.
PINNED_COMMAND_CENSUS: dict[str, int] = {
    "engine/cosmic_dragon_engine/terminal/restore.rs": 1,
    "platform/update.rs": 2,
}

COMMAND_RE = re.compile(r"\bprocess::Command\b")
SECTION_RE = re.compile(r"^\[(.+?)\]\s*$", re.MULTILINE)
DEP_NAME_RE = re.compile(r"^([A-Za-z0-9_-]+)\s*=", re.MULTILINE)


def manifest_dependencies(cargo_toml: Path) -> tuple[frozenset[str], frozenset[str]]:
    """Collect the direct dependency names from the manifest.

    Runtime names come from [dependencies] plus every
    [target.'...'.dependencies] table; dev names come from
    [dev-dependencies]. build-dependencies (there are none today)
    would be a drift finding via the same law if one appears.
    """
    text = cargo_toml.read_text(encoding="utf-8")
    runtime: set[str] = set()
    dev: set[str] = set()
    sections = list(SECTION_RE.finditer(text))
    for idx, match in enumerate(sections):
        header = match.group(1).strip()
        body_start = match.end()
        body_end = sections[idx + 1].start() if idx + 1 < len(sections) else len(text)
        body = text[body_start:body_end]
        if header == "dependencies" or (
            header.startswith("target.") and header.endswith(".dependencies")
        ):
            runtime.update(DEP_NAME_RE.findall(body))
        elif header == "dev-dependencies":
            dev.update(DEP_NAME_RE.findall(body))
        elif header == "build-dependencies":
            # A build-dependency is still a declared identity surface;
            # treat it as runtime drift so it cannot enter silently.
            runtime.update(DEP_NAME_RE.findall(body))
    return frozenset(runtime), frozenset(dev)


def sweep_source(src_root: Path) -> tuple[dict[str, list[str]], dict[str, int]]:
    """Sweep src/ for capability markers and the Command census.

    Returns (family -> [file:line findings], file -> Command count).
    Comment lines (stripped form starts with `//`) are prose, not
    surface, and never count.
    """
    findings: dict[str, list[str]] = {}
    command_census: dict[str, int] = {}
    for path in sorted(src_root.rglob("*.rs")):
        rel = str(path.relative_to(src_root)).replace("\\", "/")
        command_count = 0
        for lineno, line in enumerate(
            path.read_text(encoding="utf-8", errors="replace").splitlines(), 1
        ):
            stripped = line.lstrip()
            if stripped.startswith("//"):
                continue
            for family, pattern in CAPABILITY_MARKERS.items():
                if pattern.search(line):
                    findings.setdefault(family, []).append(f"{rel}:{lineno}")
            if COMMAND_RE.search(line):
                command_count += 1
        if command_count:
            command_census[rel] = command_count
    return findings, command_census


def main() -> int:
    repo_root = Path(__file__).resolve().parents[2]
    manifest = repo_root / "Cargo.toml"
    src_root = repo_root / "src"
    if not manifest.is_file():
        print(f"scope-census: Cargo.toml not found at {manifest}", file=sys.stderr)
        return 2
    if not src_root.is_dir():
        print(f"scope-census: src tree not found at {src_root}", file=sys.stderr)
        return 2

    problems: list[str] = []

    runtime, dev = manifest_dependencies(manifest)
    if runtime != PINNED_DEPENDENCIES:
        added = sorted(runtime - PINNED_DEPENDENCIES)
        removed = sorted(PINNED_DEPENDENCIES - runtime)
        if added:
            problems.append(
                "NEW direct dependency(ies): "
                + ", ".join(added)
                + " - audit it against the never-be contract, then (only "
                "if the owner approves) re-land PINNED_DEPENDENCIES"
            )
        if removed:
            problems.append(
                "REMOVED direct dependency(ies): "
                + ", ".join(removed)
                + " - an improvement, but the pin is stale: re-land "
                "PINNED_DEPENDENCIES in scripts/gates/scope-census.py"
            )
    if dev != PINNED_DEV_DEPENDENCIES:
        added = sorted(dev - PINNED_DEV_DEPENDENCIES)
        removed = sorted(PINNED_DEV_DEPENDENCIES - dev)
        if added:
            problems.append(
                "NEW dev dependency(ies): "
                + ", ".join(added)
                + " - audit it against the never-be contract, then (only "
                "if the owner approves) re-land PINNED_DEV_DEPENDENCIES"
            )
        if removed:
            problems.append(
                "REMOVED dev dependency(ies): "
                + ", ".join(removed)
                + " - re-land PINNED_DEV_DEPENDENCIES"
            )

    findings, command_census = sweep_source(src_root)
    for family, hits in sorted(findings.items()):
        problems.append(
            f"CAPABILITY MARKER ({family}): "
            + ", ".join(hits[:8])
            + (" ..." if len(hits) > 8 else "")
        )

    for file, count in sorted(command_census.items()):
        pinned = PINNED_COMMAND_CENSUS.get(file)
        if pinned is None:
            problems.append(
                f"NEW child-process site: {file} carries {count} "
                "process::Command line(s) and is not in the pinned "
                "census - audit it, then (only if the owner approves) "
                "add it to PINNED_COMMAND_CENSUS"
            )
        elif count > pinned:
            problems.append(
                f"GROWN child-process site: {file} went {pinned} -> "
                f"{count} - unaudited growth is a finding"
            )
        elif count < pinned:
            problems.append(
                f"SHRUNK child-process site: {file} went {pinned} -> "
                f"{count} - re-land PINNED_COMMAND_CENSUS"
            )
    for file, pinned in sorted(PINNED_COMMAND_CENSUS.items()):
        if file not in command_census:
            problems.append(
                f"VACATED child-process site: {file} now carries zero "
                "process::Command lines - retire it from "
                "PINNED_COMMAND_CENSUS"
            )

    if problems:
        print("scope-census: DRIFT from the never-be contract")
        for problem in problems:
            print(f"  - {problem}")
        print(
            f"scope-census: manifest carries {len(runtime)} runtime + "
            f"{len(dev)} dev direct dependencies; pin carries "
            f"{len(PINNED_DEPENDENCIES)} + {len(PINNED_DEV_DEPENDENCIES)}"
        )
        return 1

    print(
        "scope-census: OK - "
        f"{len(runtime)} runtime + {len(dev)} dev direct dependencies, "
        "exactly the pinned local-capability set; zero capability "
        "markers in src/ (sockets, network stacks, audio, GUI, plugin "
        "loaders, ML); the process::Command census sits at its two "
        "sanctioned sites; the never-be contract holds"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
