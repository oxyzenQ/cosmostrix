#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
"""Lock census pin (night-frontier-Z6).

On 2026-08-20 the arrayref supply-chain attack landed through the
shape this gate exists to kill: a compromised maintainer republished
a trusted crate (arrayref 0.3.10) with a NEW dependency on a
freshly-published payload crate (proc-macro1), and the malicious
family lived on crates.io for about ninety minutes. cargo-audit and
cargo-deny catch a crate once an advisory exists; the ninety-minute
window is exactly the gap BEFORE an advisory exists. The structural
counter is a census: the crate-name surface of Cargo.lock is a
closed, audited list, and ANY name that is not on it - the payload
crate's entry move - is a finding the moment it lands in the lock,
advisory or no advisory.

Two pins, both deterministic:

  1. The crate-name set (PINNED_LOCK_NAMES below). A new name in the
     graph is a finding: audit it against the supply-chain posture,
     then (only if the owner approves) re-land the pin. A name
     leaving the graph is an improvement, but the pin is stale:
     re-land it.
  2. The package-entry count (PINNED_PACKAGE_COUNT). A second
     version line of a KNOWN name bumps the count without touching
     the name set (the signal-hook 0.3/0.4 duplicate shape); that is
     also a finding - duplicate lines are deny.toml's business, and
     every one of them must be conscious.

Version drift inside a known name and inside the pinned count is
NOT a finding: Cargo.lock pins exact versions with checksums, CI
builds are --locked, and routine `cargo update` is how advisories
get consumed. The census guards the STRUCTURE of the graph, not its
version numbers.

As a belt for seats without cargo-deny, the sweep also hard-fails
on the named attack family itself (MALICIOUS_LOCK_NAMES) - any
version of any of them is the 2026-08-20 payload or its kin, with
no legitimate route into this graph.

Exit codes: 0 = census matches the pins; 1 = drift (a new or removed
crate name, a changed package-entry count, or a malicious-family
crate in the lock); 2 = usage or tree fault.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

LOCK_NAME_RE = re.compile(r'^name = "(.+?)"$', re.MULTILINE)

# The 2026-08-20 arrayref attack family (Rust security response,
# blog.rust-lang.org 2026-08-20): the payload crates and the
# compromised-author republishes. Any version of any of them is a
# hard finding - the advisory class is irrelevant, the names are the
# kill list.
MALICIOUS_LOCK_NAMES: frozenset[str] = frozenset(
    {
        "append-only-vec",
        "arone",
        "aronenao",
        "arrayref",
        "aovine",
        "internment",
        "proc-macro-en",
        "proc-macro1",
        "tinymember",
    }
)

# The pinned crate-name census, generated from Cargo.lock at the
# night-frontier-Z6 SHA. One entry per unique crate name across the
# whole resolved graph (multi-version duplicates of a listed name
# ride the package-count pin below). The set is the audited surface;
# it can only change on purpose.
PINNED_LOCK_NAMES: frozenset[str] = frozenset(
    {
        "aho-corasick",
        "anstream",
        "anstyle",
        "anstyle-parse",
        "anstyle-query",
        "anstyle-wincon",
        "arrayvec",
        "autocfg",
        "bitflags",
        "bitvec",
        "blake3",
        "block-buffer",
        "block2",
        "cc",
        "cfg-if",
        "cfg_aliases",
        "chacha20",
        "clap",
        "clap_builder",
        "clap_derive",
        "clap_lex",
        "colorchoice",
        "const-oid",
        "constant_time_eq",
        "core_detect",
        "cosmostrix",
        "cpufeatures",
        "crossterm",
        "crossterm_winapi",
        "crypto-common",
        "ctrlc",
        "digest",
        "dispatch2",
        "document-features",
        "errno",
        "find-msvc-tools",
        "funty",
        "generator",
        "getrandom",
        "heck",
        "hybrid-array",
        "inotify",
        "inotify-sys",
        "is_terminal_polyfill",
        "kqueue",
        "kqueue-sys",
        "lazy_static",
        "libc",
        "linux-raw-sys",
        "litrs",
        "lock_api",
        "log",
        "loom",
        "matchers",
        "memchr",
        "mio",
        "nix",
        "notify",
        "notify-types",
        "nu-ansi-term",
        "num-traits",
        "objc2",
        "objc2-core-foundation",
        "objc2-core-services",
        "objc2-encode",
        "once_cell",
        "once_cell_polyfill",
        "parking_lot",
        "parking_lot_core",
        "pin-project-lite",
        "proc-macro2",
        "proptest",
        "quote",
        "r-efi",
        "radium",
        "rand",
        "rand_core",
        "rand_xorshift",
        "redox_syscall",
        "regex-automata",
        "regex-syntax",
        "rustix",
        "rustversion",
        "same-file",
        "scoped-tls",
        "scopeguard",
        "sha2",
        "sharded-slab",
        "shlex",
        "signal-hook",
        "signal-hook-mio",
        "signal-hook-registry",
        "smallvec",
        "strsim",
        "syn",
        "tap",
        "thread_local",
        "tracing",
        "tracing-core",
        "tracing-log",
        "tracing-subscriber",
        "typenum",
        "unarray",
        "unicode-ident",
        "unicode-width",
        "utf8parse",
        "valuable",
        "walkdir",
        "wasi",
        "winapi",
        "winapi-i686-pc-windows-gnu",
        "winapi-util",
        "winapi-x86_64-pc-windows-gnu",
        "windows-core",
        "windows-implement",
        "windows-interface",
        "windows-link",
        "windows-result",
        "windows-strings",
        "windows-sys",
        "wyz",
    }
)

# The pinned package-entry count (Cargo.lock [[package]] blocks -
# 124 at the pin SHA, covering 121 unique names plus the three
# documented multi-version duplicates that deny.toml's skip table
# carries: signal-hook 0.3/0.4, syn 1/2, windows-link).
PINNED_PACKAGE_COUNT: int = 124


def parse_lock(lock_path: Path) -> tuple[set[str], int]:
    """Collect the crate-name set and the package-entry count."""
    text = lock_path.read_text(encoding="utf-8")
    names = set(LOCK_NAME_RE.findall(text))
    count = len(re.findall(r"^\[\[package\]\]$", text, re.MULTILINE))
    if count == 0:
        # A v3 lock always carries [[package]] blocks; zero means the
        # parse assumption broke - fail loudly rather than pass.
        count = -1
    return names, count


def main() -> int:
    repo_root = Path(__file__).resolve().parents[2]
    lock_path = repo_root / "Cargo.lock"
    if not lock_path.is_file():
        print(f"lock-census: Cargo.lock not found at {lock_path}", file=sys.stderr)
        return 2

    names, count = parse_lock(lock_path)
    if count < 0:
        print(
            "lock-census: Cargo.lock parse found zero [[package]] blocks",
            file=sys.stderr,
        )
        return 2

    problems: list[str] = []

    malicious = sorted(names & MALICIOUS_LOCK_NAMES)
    if malicious:
        problems.append(
            "MALICIOUS-FAMILY CRATE in the lock (the 2026-08-20 arrayref "
            "attack family): "
            + ", ".join(malicious)
            + " - remove it, re-resolve the graph, and treat the build "
            "host as exposed until proven clean"
        )

    added = sorted(names - PINNED_LOCK_NAMES)
    removed = sorted(PINNED_LOCK_NAMES - names)
    if added:
        problems.append(
            "NEW crate name(s) in the graph: "
            + ", ".join(added)
            + " - audit each against the supply-chain posture (the "
            "arrayref shape is a trusted crate growing a NEW dependency), "
            "then (only if the owner approves) re-land PINNED_LOCK_NAMES"
        )
    if removed:
        problems.append(
            "REMOVED crate name(s): "
            + ", ".join(removed)
            + " - an improvement, but the pin is stale: re-land "
            "PINNED_LOCK_NAMES in scripts/gates/lock-census.py"
        )
    if count != PINNED_PACKAGE_COUNT:
        direction = "GROWN" if count > PINNED_PACKAGE_COUNT else "SHRUNK"
        problems.append(
            f"{direction} package-entry count: {PINNED_PACKAGE_COUNT} -> {count}"
            + " - a new or retired version line of a known name; "
            "duplicate lines belong in deny.toml's skip table with a "
            "reason, so make the change conscious"
        )

    if problems:
        print("lock-census: DRIFT from the pinned graph")
        for problem in problems:
            print(f"  - {problem}")
        print(
            f"lock-census: lock carries {len(names)} crate name(s) across "
            f"{count} package entries; pin carries {len(PINNED_LOCK_NAMES)} "
            f"names across {PINNED_PACKAGE_COUNT} entries"
        )
        return 1

    print(
        "lock-census: OK - "
        f"{len(names)} crate name(s) across {count} package entries, "
        "exactly the pinned graph; zero malicious-family crates (the "
        "2026-08-20 arrayref attack family); the lock's structure can "
        "only change on purpose"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
