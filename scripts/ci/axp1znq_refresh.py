#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# PLATFORM: UNIX-only (Linux, macOS, BSD). CI runs on Linux runners.
#
# COSMOSTRIX AXP1-ZnQ REFRESHER - the Sunday repos-for-ci catalog pass
# (Architect's eXecution Protocol 1, night-learning-into-darkness-1)
#
"""The catalog half of AXP1-ZnQ (Architect's eXecution Protocol 1,
night-learning-into-darkness-1): every Sunday 10:00 WIB the
maintenance lane re-probes the whole repos-for-ci catalog, keeps the
addresses that answer (and serve a fresh artifact where the family's
policy asks for one), and writes the file back so the next week's
failover starts from a tested pool - the same cadence the dependency
update rides, sequenced after it so the one weekly commit carries both.

The write law (mature LTS, minimal owner intervention):

  - survivors keep their authored order - the catalog's shape is the
    owner's trust ranking (canonical first, mirrors after), never a
    latency leaderboard a noisy runner reshuffles weekly;
  - a dead or stale address becomes a dated comment in place
    (`# down 2026-11-27 (unreachable ...) - https://...`), so the
    history stays readable and the owner's next edit can revive it by
    deleting one line;
  - each file is bounded by the 100-line cap: over the cap, the OLDEST
    dated tombstones are pruned first, an active address is never
    pruned;
  - the never-empty guard: a catalog with zero live addresses is NOT
    rewritten (a flaky runner probe must never erase the owner's
    pool) - the lane warns (::warning:: in CI) and the note names CALL
    THE OWNER, the one critical the refresher escalates;
  - the band law (the owner's min 10, max 100 active addresses across
    the whole catalog): a shortfall or an overflow is a loud warning
    and a CALL THE OWNER note - the bot never invents an address to
    fill the band and never prunes an active one to shrink it;
  - freshness: an HTTP address probed through the file's own
    `# probe-path:` directive reads the artifact's Last-Modified
    header; an artifact older than the file's `# max-age-days:` policy
    is STALE and fails the write (alive-but-frozen is a lie the
    catalog must not keep - but families whose artifact is legitimately
    frozen declare `# max-age-days: off` and are judged on aliveness
    alone). A file with no probe-path directive probes the bare root,
    and a root page's Last-Modified is not the lane's artifact - so
    freshness checking turns itself off for such a file; the witness
    must be what the lane consumes, never an incidental landing page;
  - idempotent: unchanged catalog, unchanged file - no write, no
    commit-note, no churn;
  - the refresher never blocks the ship lane: it exits 0 with its
    warnings recorded, exit 1 only when the bot itself cannot run.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

from axp1znq_probe import (
    VERDICT_DOWN,
    VERDICT_PASS,
    parse_catalog,
    parse_directives,
    probe_entry,
)

DOWN_LINE = re.compile(r"^# down (\d{4}-\d{2}-\d{2}) ")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "AXP1-ZnQ (Architect's eXecution Protocol 1) refresher: re-probe a "
            "repos-for-ci catalog tree and write back the tested survivors."
        )
    )
    parser.add_argument("--root", default="repos-for-ci", help="catalog directory")
    parser.add_argument("--min-active", type=int, default=10, help="band floor, whole catalog")
    parser.add_argument("--max-active", type=int, default=100, help="band ceiling, whole catalog")
    parser.add_argument("--max-lines", type=int, default=100, help="per-file line cap")
    parser.add_argument("--attempts", type=int, default=2, help="probe attempts per address")
    parser.add_argument("--connect-timeout", type=float, default=4.0, help="TCP connect timeout s")
    parser.add_argument("--total-timeout", type=float, default=15.0, help="whole-request timeout s")
    parser.add_argument("--workers", type=int, default=8, help="concurrent probes")
    parser.add_argument(
        "--note-file",
        default="",
        help="commit-note path to write when the catalog moved (maintenance lane)",
    )
    parser.add_argument(
        "--manifest-file",
        default="",
        help="exact changed-path list to write (the maintenance lane stages these, never a glob)",
    )
    parser.add_argument("--dry-run", action="store_true", help="report only, write nothing")
    parser.add_argument(
        "--selftest",
        action="store_true",
        help="run the offline self-checks (no network) and exit",
    )
    return parser


def dated_comment(today: str, detail: str, target: str) -> str:
    """The in-place tombstone a failed address leaves behind."""
    compact = " ".join(str(detail).split())
    return f"# down {today} ({compact}) - {target}"


def prune_oldest_dead(lines: list, cap: int) -> list:
    """Drop dated tombstones oldest-first until the file fits the cap."""
    while len([line for line in lines if line.strip()]) > cap:
        dated = [
            (index, match.group(1))
            for index, line in enumerate(lines)
            if (match := DOWN_LINE.match(line.strip()))
        ]
        if not dated:
            break
        oldest = min(dated, key=lambda pair: pair[1])[0]
        del lines[oldest]
    return lines


def refresh_one(path: Path, args: argparse.Namespace, today: str) -> dict:
    """Probe one catalog file under its own directives; return the record."""
    original = path.read_text(encoding="utf-8")
    entries = parse_catalog(original)
    directives = parse_directives(original)
    banner = "AXP1-ZnQ (Architect's eXecution Protocol 1) refresher"
    if not entries:
        return {"path": path, "changed": False, "critical": True, "wrote": False,
                "summary": f"{banner}: {path.name} carries no active address - CALL THE OWNER"}

    with concurrent.futures.ThreadPoolExecutor(max_workers=min(args.workers, len(entries))) as pool:
        # Freshness rides the file's declared artifact: with no
        # # probe-path: directive the probe hits the bare root, whose
        # Last-Modified dates an incidental landing page, not anything
        # the lane consumes - so the age policy turns itself off there.
        effective_max_age = (
            directives.max_age_days if directives.probe_path else None
        )
        futures = {
            pool.submit(
                probe_entry,
                entry,
                directives.probe_path,
                args.connect_timeout,
                args.total_timeout,
                args.attempts,
                effective_max_age,
            ): entry
            for entry in entries
        }
        verdicts = {
            futures[future]: future.result()
            for future in concurrent.futures.as_completed(futures)
        }

    alive = sum(1 for result in verdicts.values() if result.verdict == VERDICT_PASS)
    stale = sum(1 for result in verdicts.values() if result.verdict not in (VERDICT_PASS, VERDICT_DOWN))
    down = sum(1 for result in verdicts.values() if result.verdict == VERDICT_DOWN)

    if alive == 0:
        return {"path": path, "changed": False, "critical": True, "wrote": False,
                "summary": (f"{banner}: {path.name} 0 alive of {len(entries)} probed "
                            f"({down} down, {stale} stale) - catalog untouched, CALL THE OWNER")}

    new_lines = []
    for raw in original.splitlines():
        entry = next((candidate for candidate in entries if candidate.raw == raw), None)
        if entry is None:
            new_lines.append(raw)
            continue
        result = verdicts[entry]
        if result.verdict == VERDICT_PASS:
            new_lines.append(raw)
        else:
            new_lines.append(dated_comment(today, result.detail, entry.target))
    new_lines = prune_oldest_dead(new_lines, args.max_lines)
    new_text = "\n".join(new_lines) + ("\n" if original.endswith("\n") else "")

    changed = new_text != original
    policy = directives.probe_path or "(root)"
    summary = (
        f"{banner}: {path.name} {alive} alive, {down} down, {stale} stale "
        f"(probe path {policy}, {'rewrote' if changed else 'no change'})"
    )
    wrote = False
    if changed and not args.dry_run:
        path.write_text(new_text, encoding="utf-8")
        wrote = True
    return {"path": path, "changed": changed, "critical": False, "wrote": wrote,
            "summary": summary}


def selftest() -> int:
    """Offline self-checks: tombstone shape, prune order, band law math."""
    body = (
        "# purpose: unit test\n"
        "# probe-path: dists/noble/InRelease\n"
        "# max-age-days: off\n"
        "\n"
        "https://keep.example.com/ubuntu\n"
        "https://kill.example.com/ubuntu\n"
    )
    entries = parse_catalog(body)
    assert len(entries) == 2, entries
    directives = parse_directives(body)
    assert directives.max_age_days is None, directives.max_age_days
    assert directives.probe_path == "dists/noble/InRelease", directives.probe_path

    tombstone = dated_comment("2026-11-27", "unreachable (multi  line\nreason)", "https://x")
    assert tombstone == "# down 2026-11-27 (unreachable (multi line reason)) - https://x", tombstone

    lines = ["a", "# down 2025-01-01 (old) - x", "b", "# down 2024-01-01 (older) - y", "c"]
    pruned = prune_oldest_dead(list(lines), 4)
    assert "# down 2024-01-01 (older) - y" not in pruned, pruned
    assert "# down 2025-01-01 (old) - x" in pruned, pruned
    pruned_fully = prune_oldest_dead(list(lines), 2)
    assert pruned_fully == ["a", "b", "c"], pruned_fully  # actives are never pruned

    band_ok = 10 <= 14 <= 100
    band_low = 9 < 10
    assert band_ok and band_low
    print("AXP1-ZnQ refresher selftest: PASS (tombstone, prune order, band law)")
    return 0


def main() -> int:
    args = build_parser().parse_args()
    if args.selftest:
        return selftest()
    root = Path(args.root)
    if not root.is_dir():
        print(f"AXP1-ZnQ refresher: {root} is not a directory", file=sys.stderr)
        return 1
    catalogs = sorted(root.glob("*.txt"))
    if not catalogs:
        print(f"AXP1-ZnQ refresher: no *.txt catalog under {root}", file=sys.stderr)
        return 1

    today = datetime.now(timezone.utc).date().isoformat()
    records = [refresh_one(path, args, today) for path in catalogs]

    for record in records:
        print(record["summary"])
        if record["critical"]:
            print("::warning::AXP1-ZnQ address refresh hit a critical state - CALL THE OWNER")

    active_total = 0
    for path in catalogs:
        active_total += len(parse_catalog(path.read_text(encoding="utf-8")))
    if active_total < args.min_active:
        print(
            f"AXP1-ZnQ refresher: {active_total} active address(es) across the catalog is "
            f"below the owner's floor of {args.min_active} - the bot never invents an "
            f"address; CALL THE OWNER to add candidates"
        )
        print("::warning::AXP1-ZnQ catalog below the 10-address floor - CALL THE OWNER")
    elif active_total > args.max_active:
        print(
            f"AXP1-ZnQ refresher: {active_total} active address(es) exceed the owner's "
            f"ceiling of {args.max_active} - the bot never prunes an active address; "
            f"CALL THE OWNER to trim the pool"
        )
        print("::warning::AXP1-ZnQ catalog above the 100-address ceiling - CALL THE OWNER")

    moved = [record for record in records if record["changed"]]
    if moved and not args.dry_run and args.note_file:
        note = "\n".join(record["summary"] for record in moved)
        Path(args.note_file).write_text(note + "\n", encoding="utf-8")
        print(f"AXP1-ZnQ refresher: commit note written to {args.note_file}")
    if moved and not args.dry_run and args.manifest_file:
        manifest = "\n".join(str(record["path"]) for record in moved)
        Path(args.manifest_file).write_text(manifest + "\n", encoding="utf-8")
        print(f"AXP1-ZnQ refresher: changed-path manifest written to {args.manifest_file}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
