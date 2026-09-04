#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""Stale-reference and duplicate-comment hunter for src/**.rs and test/**.rs.

Why not grep: grep matches the WHOLE file, so a flag mentioned inside a
string literal, an attribute, or the REMOVED_FLAGS registry itself
produces the same hit as a stale prose comment. This script parses Rust
comment structure (line/doc/block comments vs string literals vs code)
and cross-verifies every reference against reality:

  1. CLI flags (``--foo``): verified against the live clap surface
     (derived long names from the Args struct + explicit long/short
     attrs + help strings) and the REMOVED_FLAGS registry. A comment
     referencing a flag that (a) is in REMOVED_FLAGS outside the
     intentional zones, or (b) does not exist anywhere on the CLI
     surface, is STALE.
  2. File paths (src/x.rs, docs/x.md, ...): verified against the
     filesystem.
  3. crate:: module paths: the module prefix is resolved through the
     module tree (dir/mod.rs or file.rs); an unresolvable prefix is
     STALE.
  4. Duplicate comment lines: normalized lines (>= MIN_DUP_LEN chars)
     repeated within a file, or verbatim multi-line comment blocks
     repeated across files - candidates for de-duplication.

Intentional zones (never flagged): src/validation/mod.rs holds the
REMOVED_FLAGS registry; src/main.rs prevalidation legitimately mentions
removed flags in its implementation comments.

NIGHT-hunter-5 (2026-09-05): the scan corpus covers the mirrored
test/ tree (NIGHT-hunter-1) in addition to src/ — its files are real
source carrying the same comment contracts. Migration-history
comments that name pre-move paths keep their negation-context
exemption ("Previously these were flat files at src/ root" in
test/tests/mod.rs is intentional history, not staleness).

Usage: python3 scripts/stale-hunt.py [--min-dup-len N]
"""

import re
from collections import defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
MIN_DUP_LEN = 45

INTENTIONAL_REMOVED_FLAG_FILES = {
    "src/validation/mod.rs",  # REMOVED_FLAGS registry + did-you-mean examples
    "src/main.rs",  # prevalidation + argv expansion + typo examples
    "src/cli/app.rs",  # field definitions may document removed flags
}

# NIGHT-hunter-5: scan BOTH trees — src/ (production) and test/
# (the mirrored test module tree from NIGHT-hunter-1, included into
# src modules via #[path] attributes; its comments reference the same
# flags, paths, and crate:: modules and must not go stale either).
RUST_FILES = sorted((REPO / "src").rglob("*.rs")) + sorted(
    (REPO / "test").rglob("*.rs")
)


# ── Rust comment extraction (skips strings + attrs) ───────────────────────


def strip_strings_keep_comments(text: str) -> str:
    """Blank out string/char literal CONTENTS but keep comment text.

    Two-pass state machine: tracks whether we are inside a // comment,
    /* */ comment, "string", 'char', or raw string r#"..."#. Comment text
    survives; literal contents become spaces (so flags inside help-string
    literals do not produce false stale hits).
    """
    out = []
    i = 0
    n = len(text)
    state = "code"  # code | line_comment | block_comment | string | raw_string
    raw_hashes = 0

    while i < n:
        ch = text[i]
        nxt = text[i + 1] if i + 1 < n else ""

        if state == "code":
            if ch == "/" and nxt == "/":
                state = "line_comment"
                out.append("//")
                i += 2
                continue
            if ch == "/" and nxt == "*":
                state = "block_comment"
                out.append("/*")
                i += 2
                continue
            if ch == '"':
                state = "string"
                out.append(" ")
                i += 1
                continue
            if ch == "r" and nxt in '#"':
                # raw string r"..." or r#"..."#
                m = re.match(r'r(#*)"', text[i:])
                if m:
                    raw_hashes = len(m.group(1))
                    state = "raw_string"
                    out.append(" " * len(m.group(0)))
                    i += len(m.group(0))
                    continue
            if ch == "'":
                # char literal or lifetime - treat single-char as literal,
                # lifetime ('a) passes through as code (harmless)
                m = re.match(r"'(\\.|[^'\\])'", text[i:])
                if m:
                    out.append(" " * len(m.group(0)))
                    i += len(m.group(0))
                    continue
            out.append(ch)
            i += 1
        elif state == "line_comment":
            if ch == "\n":
                state = "code"
                out.append("\n")
            else:
                out.append(ch)
            i += 1
        elif state == "block_comment":
            if ch == "*" and nxt == "/":
                state = "code"
                out.append("*/")
                i += 2
                continue
            out.append(ch)
            i += 1
        elif state == "string":
            if ch == "\\":
                out.append("  ")
                i += 2
                continue
            if ch == '"':
                state = "code"
                out.append(" ")
            else:
                out.append(" ")
            i += 1
        elif state == "raw_string":
            closer = '"' + "#" * raw_hashes
            if text.startswith(closer, i):
                state = "code"
                out.append(" " * len(closer))
                i += len(closer)
                continue
            out.append(" ")
            i += 1
    return "".join(out)


def comment_spans(text: str):
    """Yield (start_offset, comment_text) for each comment in `text`.

    Consecutive `//` lines are merged into ONE span: a sentence like
    "--foo was removed" split across two comment lines must still read
    as negation context for the flag on either line.
    """
    cleaned = strip_strings_keep_comments(text)
    raw = list(re.finditer(r"//[^\n]*|/\*.*?\*/", cleaned, re.DOTALL))
    merged = []
    for m in raw:
        if (
            merged
            and m.group(0).startswith("//")
            and merged[-1][1].startswith("//")
            and cleaned[merged[-1][0] + len(merged[-1][1]) : m.start()].strip() == ""
        ):
            merged[-1] = (merged[-1][0], merged[-1][1] + "\n" + m.group(0))
        else:
            merged.append((m.start(), m.group(0)))
    yield from merged


# ── CLI surface ground truth ───────────────────────────────────────────────


def collect_live_flags() -> set[str]:
    """Every long flag the current CLI accepts, from the Args derive."""
    # The Args derive lives in src/config/mod.rs (verified 2026-08-24;
    # scanning cli/app.rs only was the v1 bug - that file holds the
    # derived CloudConfig, not the clap surface).
    sources = [
        REPO / "src/config/mod.rs",
        REPO / "src/cli/mod.rs",
        REPO / "src/cli/app.rs",
        REPO / "src/cli/cli_parse.rs",
    ]
    flags = set()
    for path in sources:
        if not path.exists():
            continue
        src = path.read_text(errors="replace")
        for m in re.finditer(r'long\s*=\s*"([a-z0-9\-]+)"', src):
            flags.add("--" + m.group(1))
        for m in re.finditer(r'long\s*\(\s*"([a-z0-9\-]+)"', src):
            flags.add("--" + m.group(1))
        for m in re.finditer(r"^\s*pub\s+([a-z_][a-z0-9_]*)\s*:", src, re.MULTILINE):
            flags.add("--" + m.group(1).replace("_", "-"))
        strings = re.findall(r'"([^"\n]*)"', src)
        for s in strings:
            for f in re.findall(r"--[a-z0-9][a-z0-9\-]+", s):
                flags.add(f)
    flags |= {"--help", "--version"}
    return flags


# Flags of EXTERNAL tools that legitimately appear in comments (cargo,
# rustup, git, gpg, pip, nextest...). These are not cosmostrix CLI flags
# and are never stale cosmostrix references.
EXTERNAL_TOOL_FLAGS = {
    "--release",
    "--dev",
    "--locked",
    "--quiet",
    "--all-targets",
    "--all-features",
    "--nocapture",
    "--bin",
    "--profile",
    "--target",
    "--jobs",
    "--features",
    "--no-default-features",
    "--example",
    "--test",
    "--lib",
    "--benches",
    "--manifest-path",
    "--package",
    "--ignored",
    "--short",
    "--tool",
    "--system",
    "--depth",
    "--single-branch",
    "--no-tags",
    "--recurse-submodules",
    "--keyserver",
    "--recv-keys",
    "--send-keys",
    "--list-keys",
    "--with-colons",
    "--batch",
    "--yes",
    "--detach-sign",
    "--armor",
    "--local-user",
    "--pinentry-mode",
    "--passphrase-fd",
    "--output",
    "--verify",
    "--install",
    "--user",
    "--break-system-packages",
    "--check",
    "--fix",
    "--config",
    "--ignore",
    "--skip",
    "--amend",
    "--no-edit",
    "--force",
    "--set-upstream",
    "--ff-only",
    "--stat",
    "--oneline",
    "--auto",
    "--validate",
    "--resume",
    "--full",
    "--filter",
    "--no-install",
    "--quiet-miri",
    "--verbose",
    "--bench",
    "--suite",
    "--file",
    "--line",
    "--message",
}

PLACEHOLDER_FLAGS = {"--foo", "--flag", "--bar", "--baz"}


def collect_removed_flags() -> set[str]:
    """Flags in the REMOVED_FLAGS registry (intentionally documented)."""
    v = (REPO / "src/validation/mod.rs").read_text(errors="replace")
    m = re.search(r"REMOVED_FLAGS[^=]*=\s*&\[", v)
    if not m:
        return set()
    body = v[m.end() :]
    end = body.find("];")
    body = body[:end]
    return set(re.findall(r'"(--[a-z0-9\-]+)"', body))


# ── Module-tree resolution for crate:: paths ───────────────────────────────


REEXPORTED_NAMES: set[str] = set()


def collect_reexported_names() -> set[str]:
    """Crate-root names bound by `pub use` in src/main.rs."""
    names: set[str] = set()
    main = REPO / "src/main.rs"
    if not main.exists():
        return names
    src = main.read_text(errors="replace")
    for m in re.finditer(r"pub(?:\(crate\))?\s+use\s+([\w:]+)\s*;", src):
        names.add(m.group(1).split("::")[-1])
    for m in re.finditer(
        r"pub(?:\(crate\))?\s+use\s+([\w:]+)::\{([^}]*)\}", src, re.DOTALL
    ):
        for item in m.group(2).split(","):
            item = item.strip()
            if item:
                names.add(item.split(" as ")[-1].strip())
    # Glob re-exports: `pub use N::*` exposes every `pub mod X` inside N,
    # so crate::X resolves. Expand by scanning N's directory.
    for m in re.finditer(r"pub(?:\(crate\))?\s+use\s+([\w:]+)::\*", src):
        mod_name = m.group(1).split("::")[-1]
        for cand in (
            REPO / "src" / mod_name / "mod.rs",
            REPO / "src" / (mod_name + ".rs"),
        ):
            if cand.exists():
                body = cand.read_text(errors="replace")
                for mm in re.finditer(
                    r"^\s*pub(?:\(crate\))?\s+mod\s+(\w+)", body, re.MULTILINE
                ):
                    names.add(mm.group(1))
                break
    return names


def module_exists(path: str) -> bool:
    """Resolve a crate::a::b prefix through src/ module layout."""
    segs = [s for s in path.split("::") if s]
    if not segs or segs[0] != "crate":
        return True  # not a crate path; nothing to verify
    segs = segs[1:]
    # Glob/placeholder patterns: `crate::bench_X::Foo`.
    if any(s.endswith("_") or "*" in s for s in segs):
        return True
    # First segment may be a crate-root re-export (module lives elsewhere).
    if len(segs) == 1 and segs[0] in REEXPORTED_NAMES:
        return True
    # Re-exported head + deeper path: resolve the TAIL item globally
    # (the module may live far from src/<name>/ - e.g. crate::memstat::
    # current_rss_kb resolves inside sysstat/).
    if segs[0] in REEXPORTED_NAMES and len(segs) >= 2:
        tail = segs[-1]
        for f in RUST_FILES:
            if re.search(
                r"\b(?:fn|struct|enum|const|static|mod|trait|type|macro_rules!?)\s+"
                + re.escape(tail)
                + r"\b",
                f.read_text(errors="replace"),
            ) or re.search(
                r"\buse\b[^;\n]*\b" + re.escape(tail) + r"\b",
                f.read_text(errors="replace"),
            ):
                return True
        return False
    base = REPO / "src"
    for idx, seg in enumerate(segs):
        candidates = [
            base / seg / "mod.rs",
            base / (seg + ".rs"),
        ]
        if any(c.exists() for c in candidates):
            base = base / seg
            continue
        # Segment might be an item (fn/struct/const) inside the parent
        # module - verify the parent file contains the identifier.
        parent_files = [base / "mod.rs", base.with_suffix(".rs")]
        if idx > 0:
            search_files = list(parent_files)
            if base.is_dir():
                search_files.extend(base.rglob("*.rs"))
            for pf in search_files:
                if pf.exists() and (
                    re.search(
                        r"\b(?:fn|struct|enum|const|static|mod|trait|type|macro_rules!?)\s+"
                        + re.escape(seg)
                        + r"\b",
                        pf.read_text(errors="replace"),
                    )
                    or re.search(
                        r"\buse\b[^;\n]*\b" + re.escape(seg) + r"\b",
                        pf.read_text(errors="replace"),
                    )
                ):
                    return True
        return False
    return True


# ── Main scan ──────────────────────────────────────────────────────────────

FLAG_RE = re.compile(r"--[a-z][a-z0-9\-]{2,}")
PATH_RE = re.compile(
    r"(?:src|docs|scripts|benchmark|pgo-runner|aur|\.github)/"
    r"[A-Za-z0-9_\-./]+\.(?:rs|md|sh|py|toml|yml|yaml)"
)
CRATE_RE = re.compile(r"crate::[a-z_][a-z0-9_:]*")

# A comment that NEGATES a flag's existence ("--x has been removed", "no
# longer --x", "--x replaced by --y") is intentional history documentation,
# not a stale reference. This is the grep-cannot-do-this part: the scanner
# reads the sentence around the flag before judging it.
NEGATION_RE = re.compile(
    r"removed|deprecated|no\s+longer|replaced\s+by|merged\s+into|"
    r"deleted|dropped\s+in|superseded|used\s+to\s+be|"
    r"legacy|old\s+flag|obsolete|originally|absorbed|converted|"
    r"previously|formerly|replaces|merged|corrected|"
    r"used\s+to\s+\w|the\s+old",
    re.IGNORECASE,
)


def scan():
    global REEXPORTED_NAMES
    REEXPORTED_NAMES = collect_reexported_names()
    live = collect_live_flags()
    removed = collect_removed_flags()
    print(
        f"CLI surface: {len(live)} live long flags, "
        f"{len(removed)} removed registered, "
        f"{len(REEXPORTED_NAMES)} crate-root re-exports, "
        f"{len(RUST_FILES)} .rs files scanned"
    )

    stale_flags = []
    stale_paths = []
    stale_modules = []

    for f in RUST_FILES:
        rel = str(f.relative_to(REPO))
        text = f.read_text(errors="replace")
        for start, comment in comment_spans(text):
            # flags
            for fm in FLAG_RE.finditer(comment):
                flag = fm.group(0)
                if (
                    flag in live
                    or flag in EXTERNAL_TOOL_FLAGS
                    or flag in PLACEHOLDER_FLAGS
                ):
                    continue
                # Glob/placeholder patterns: `--list-*`, `--bench_X`.
                if flag.endswith(("-", "_")) or "*" in flag:
                    continue
                if rel in INTENTIONAL_REMOVED_FLAG_FILES:
                    continue
                # Negation context = intentional history documentation.
                if NEGATION_RE.search(comment):
                    continue
                # Flag neither live nor intentionally-documented
                line = text.count("\n", 0, start) + 1
                reason = (
                    "removed-flag reference"
                    if flag in removed
                    else "unknown flag (not on CLI surface)"
                )
                stale_flags.append((rel, line, flag, reason, comment.strip()[:70]))
            # paths (migration-history comments legitimately name old
            # paths - the negation context check applies here too)
            for pm in PATH_RE.finditer(comment):
                p = pm.group(0).rstrip(".")
                if not (REPO / p).exists():
                    if NEGATION_RE.search(comment):
                        continue
                    line = text.count("\n", 0, start) + 1
                    stale_paths.append((rel, line, p, comment.strip()[:70]))
            # crate modules (re-export documentation names legacy
            # paths by design - negation context applies)
            for cm in CRATE_RE.finditer(comment):
                path = cm.group(0).rstrip(":")
                if not module_exists(path):
                    if NEGATION_RE.search(comment):
                        continue
                    line = text.count("\n", 0, start) + 1
                    stale_modules.append((rel, line, path, comment.strip()[:70]))

    # ── duplicate comment lines ──────────────────────────────────────
    line_map = defaultdict(list)  # normalized line -> [(file, line_no)]
    for f in RUST_FILES:
        rel = str(f.relative_to(REPO))
        text = f.read_text(errors="replace")
        for start, comment in comment_spans(text):
            first_line = comment.split("\n")[0]
            norm = re.sub(r"\s+", " ", first_line).strip()
            norm = re.sub(r"^//+ ?|/\*+ ?", "", norm).strip()
            alnum = sum(c.isalnum() for c in norm)
            if (
                len(norm) >= MIN_DUP_LEN
                and alnum >= len(norm) * 0.5
                and not norm.startswith("SPDX")
                and "Copyright" not in norm
            ):
                line_no = text.count("\n", 0, start) + 1
                line_map[norm].append((rel, line_no))
    dups = {
        k: v for k, v in line_map.items() if len({f for f, _ in v}) > 1 or len(v) > 3
    }

    print(f"\n=== STALE CLI FLAG REFERENCES ({len(stale_flags)}) ===")
    for rel, line, flag, reason, ctx in sorted(stale_flags):
        print(f"  {rel}:{line}  {flag}  [{reason}]")
        print(f"      {ctx}")

    print(f"\n=== STALE FILE PATHS ({len(stale_paths)}) ===")
    for rel, line, p, ctx in sorted(stale_paths):
        print(f"  {rel}:{line}  {p}")
        print(f"      {ctx}")

    print(f"\n=== STALE crate:: MODULES ({len(stale_modules)}) ===")
    for rel, line, path, ctx in sorted(stale_modules):
        print(f"  {rel}:{line}  {path}")
        print(f"      {ctx}")

    print(
        f"\n=== DUPLICATE COMMENT LINES ({len(dups)} distinct, threshold {MIN_DUP_LEN} chars) ==="
    )
    for norm, locs in sorted(dups.items(), key=lambda kv: -len(kv[1])):
        files = sorted({f for f, _ in locs})
        print(f"  x{len(locs)} in {len(files)} files: {norm[:80]}")
        for f in files[:4]:
            lines = sorted(ln for ff, ln in locs if ff == f)
            print(f"      {f}: lines {lines[:6]}")
        if len(files) > 4:
            print(f"      ... and {len(files) - 4} more files")

    total = len(stale_flags) + len(stale_paths) + len(stale_modules)
    print(f"\nTOTAL stale references: {total}, duplicate groups: {len(dups)}")


if __name__ == "__main__":
    scan()
