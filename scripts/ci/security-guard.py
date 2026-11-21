#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
"""Tiered RustSec supply chain guard for the dependency maintenance lane.

Plain `cargo audit` fails on every advisory, so it cannot express the
two scenarios the Sunday maintenance lane must tell apart (owner
directive 2026-11-21, the sleeping-owner contract):

- Tier 1 (non-blocking): the advisory ships patched versions. A routine
  `cargo update` can fix it, so the lane stays green and the report
  shouts UPDATE ASAP with the advisory id, severity and fixed versions.
- Tier 2 (blocking): the advisory ships NO patched version. The
  dependency is dangerous and cannot be fixed by updating, so the guard
  exits 1 with a CALL OWNER banner. The scheduled run fails hard, the
  GitHub scheduled-failure notification reaches the owner, and the
  commit phase never publishes a lock refresh while the risk stands.

Scope follows the night-zen-1 law: the lockfile set comes from the
canonical manifest walk (`scripts/ci/update-all-workspaces.sh --list`,
the single source of truth shared with the refresh and commit phases),
so every workspace lockfile is audited, not just the root's.

Fail closed: a missing tool, a missing lockfile set, a malformed report
or any audit infrastructure error exits 1. A broken guard must never
pass silently.

Usage:
  python3 scripts/ci/security-guard.py                   # audit run
  python3 scripts/ci/security-guard.py --phase baseline  # labelled run
  python3 scripts/ci/security-guard.py --json-file R     # offline report
  python3 scripts/ci/security-guard.py --self-test       # fixture tests
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

# cargo audit documented exit codes: 0 = clean, 1 = vulnerabilities
# found (the JSON report is still authoritative), anything else is an
# infrastructure error. Only 0 and 1 may be read as a usable report.
CARGO_AUDIT_OK = 0
CARGO_AUDIT_VULNERABLE = 1

WALK_SCRIPT = "scripts/ci/update-all-workspaces.sh"

SEVERITY_RANK = {"critical": 0, "high": 1, "medium": 2, "low": 3}
SEVERITY_DEFAULT_RANK = 4


class GuardError(Exception):
    """Audit infrastructure failure; the guard fails closed on it."""


def discover_lockfiles(repo_root: Path) -> list[Path]:
    """Derive every workspace lockfile from the canonical manifest walk.

    The walk is the single source of truth for manifest discovery: the
    refresh phase consumes its default mode and the commit phase its
    --list mode, so the audit set can never drift from either.
    """
    proc = subprocess.run(
        ["bash", WALK_SCRIPT, "--list"],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        detail = proc.stderr.strip() or "no stderr output"
        raise GuardError(
            f"manifest walk failed (exit {proc.returncode}): {detail}"
        )
    locks: list[Path] = []
    for line in proc.stdout.splitlines():
        manifest = line.strip()
        if not manifest:
            continue
        if not manifest.endswith("Cargo.toml"):
            raise GuardError(f"walk emitted a non-manifest path: {manifest}")
        locks.append(Path(manifest[: -len("Cargo.toml")] + "Cargo.lock"))
    if not locks:
        raise GuardError("manifest walk found no Cargo.toml")
    return locks


def parse_report(text: str, context: str) -> dict:
    """Parse a cargo audit JSON report, tolerating noise around it."""
    start = text.find("{")
    end = text.rfind("}")
    if start == -1 or end <= start:
        raise GuardError(f"no JSON report found for {context}")
    try:
        return json.loads(text[start : end + 1])
    except json.JSONDecodeError as exc:
        raise GuardError(f"malformed JSON report for {context}: {exc}") from exc


def run_audit(repo_root: Path, lockfile: Path) -> dict:
    """Run cargo audit against one lockfile and return its JSON report."""
    proc = subprocess.run(
        ["cargo", "audit", "--json", "--file", str(lockfile)],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode not in (CARGO_AUDIT_OK, CARGO_AUDIT_VULNERABLE):
        detail = (proc.stderr.strip() or "no stderr output")[-500:]
        raise GuardError(
            f"cargo audit failed on {lockfile} "
            f"(exit {proc.returncode}): {detail}"
        )
    return parse_report(proc.stdout, context=str(lockfile))


def patched_versions(item: dict) -> list[str]:
    """Fixed-version requirements for one finding, across schema shapes.

    cargo audit 0.21 serializes the patched list at the advisory level;
    older releases carry it in the sibling versions object, and very old
    ones used a single patched_versions string. Schema drift across
    releases must never flip a tier, so all three shapes are accepted.
    """
    advisory = item.get("advisory")
    if isinstance(advisory, dict):
        value = advisory.get("patched")
        if isinstance(value, list) and value:
            return [str(v) for v in value]
    versions = item.get("versions")
    if isinstance(versions, dict):
        value = versions.get("patched")
        if isinstance(value, list) and value:
            return [str(v) for v in value]
    if isinstance(advisory, dict):
        value = advisory.get("patched_versions")
        if isinstance(value, str) and value.strip():
            return [value.strip()]
    return []


def classify(report: dict) -> tuple[list[dict], list[dict], list[dict]]:
    """Split one report into (fixable, unfixable, informational)."""
    fixable: list[dict] = []
    unfixable: list[dict] = []
    informational: list[dict] = []
    for result in (report.get("results") or {}).get("list") or []:
        for item in (result.get("vulnerabilities") or {}).get("list") or []:
            (fixable if patched_versions(item) else unfixable).append(item)
        for item in (result.get("warnings") or {}).get("list") or []:
            informational.append(item)
    return fixable, unfixable, informational


def warning_note(item: dict) -> str:
    """Best-effort warning category across cargo audit JSON shapes."""
    advisory = item.get("advisory")
    if not isinstance(advisory, dict):
        advisory = {}
    for key in ("kind", "category"):
        value = item.get(key)
        if value:
            return str(value)
    value = advisory.get("informational")
    if value:
        return str(value)
    return "warning"


def finding_row(item: dict, lockfile: Path) -> dict:
    """Flatten one finding into a display row."""
    advisory = item.get("advisory")
    if not isinstance(advisory, dict):
        advisory = {}
    aliases = [str(a) for a in advisory.get("aliases") or []]
    severity = str(advisory.get("severity") or "unknown")
    return {
        "lockfile": str(lockfile),
        "package": str(advisory.get("package") or "unknown"),
        "id": str(advisory.get("id") or "unknown"),
        "severity": severity,
        "rank": SEVERITY_RANK.get(severity.lower(), SEVERITY_DEFAULT_RANK),
        "patched": ", ".join(patched_versions(item)) or "none",
        "aliases": ", ".join(aliases) or "-",
        "title": str(advisory.get("title") or "").strip(),
        "url": str(advisory.get("url") or "").strip(),
        "note": warning_note(item),
    }


def _cell(value: object) -> str:
    """Render one table cell; escape pipes and keep empties visible."""
    text = str(value).replace("|", "\\|")
    return text if text else "-"


def _advisory_cell(row: dict) -> str:
    """Link the advisory id to its RustSec page when the url is known."""
    if row.get("url"):
        return f"[{_cell(row['id'])}]({_cell(row['url'])})"
    return _cell(row["id"])


def md_table(rows: list[dict], columns: list[tuple[str, str]]) -> str:
    """Render display rows as a GitHub markdown table."""
    header = "| " + " | ".join(name for name, _ in columns) + " |"
    divider = "|" + "|".join(" --- " for _ in columns) + "|"
    body = [
        "| " + " | ".join(_cell(row.get(key)) for _, key in columns) + " |"
        for row in rows
    ]
    return "\n".join([header, divider] + body)


def _tier_section(
    lines: list[str],
    title: str,
    banner: list[str],
    rows: list[dict],
) -> None:
    """Append one tier section grouped by lockfile."""
    lines.append(f"### {title}")
    lines.append("")
    lines.extend(banner)
    lines.append("")
    by_lock: dict[str, list[dict]] = {}
    for row in rows:
        by_lock.setdefault(row["lockfile"], []).append(row)
    columns = [
        ("Advisory", "advisory"),
        ("Package", "package"),
        ("Severity", "severity"),
        ("Fixed in", "patched"),
        ("CVE aliases", "aliases"),
        ("Title", "title"),
    ]
    for lock in sorted(by_lock):
        group = sorted(by_lock[lock], key=lambda r: (r["rank"], r["id"]))
        display = [dict(r, advisory=_advisory_cell(r)) for r in group]
        lines.append(f"Lockfile `{lock}`:")
        lines.append("")
        lines.append(md_table(display, columns))
        lines.append("")


def render_report(
    phase: str,
    audited: list[str],
    skipped: list[str],
    fixable: list[dict],
    unfixable: list[dict],
    informational: list[dict],
) -> str:
    """Build the GitHub step summary markdown and return it."""
    lines: list[str] = [f"## Security supply chain audit report ({phase})", ""]
    lines.append(
        "Scope: every workspace lockfile derived from the canonical "
        f"manifest walk (`{WALK_SCRIPT} --list`). Audited "
        f"{len(audited)} lockfile(s): "
        + (", ".join(f"`{p}`" for p in audited) if audited else "none")
        + "."
    )
    lines.append("")

    if unfixable:
        lines.extend(
            [
                "> [!CAUTION]",
                "> CALL OWNER: unfixable advisory present. No patched version",
                "> exists in the RustSec database, so a routine `cargo update`",
                "> cannot fix these dependencies. Owner decision required:",
                "> remove, replace, vendor-patch, or accept the risk. This run",
                "> fails hard so the scheduled-failure notification reaches the",
                "> owner, and the commit phase never publishes a lock refresh",
                "> while the risk stands.",
                "",
            ]
        )
        _tier_section(
            lines,
            "Tier 2: unfixable (blocking, exit 1)",
            [],
            unfixable,
        )
    if fixable:
        lines.extend(
            [
                "> [!WARNING]",
                "> UPDATE ASAP: patched versions exist, so a routine update can",
                "> fix these dependencies. Non-blocking by contract: the lane",
                "> continues, but a high-value project must not sit on",
                "> unpatched advisories.",
                "",
            ]
        )
        _tier_section(
            lines,
            "Tier 1: fixable (non-blocking)",
            [],
            fixable,
        )
    if informational:
        lines.append("### Informational (non-blocking)")
        lines.append("")
        columns = [
            ("Lockfile", "lockfile"),
            ("Advisory", "advisory"),
            ("Package", "package"),
            ("Note", "note"),
        ]
        for row in sorted(informational, key=lambda r: (r["lockfile"], r["id"])):
            display = dict(row, advisory=_advisory_cell(row))
            lines.append(md_table([display], columns))
        lines.append("")
    if not (fixable or unfixable or informational):
        lines.extend(
            [
                "> [!NOTE]",
                "> Clean: no RustSec vulnerabilities or warnings in any",
                "> audited lockfile.",
                "",
            ]
        )
    if skipped:
        lines.append("### Skipped lockfiles (non-blocking)")
        lines.append("")
        for path in skipped:
            lines.append(
                f"- `{path}`: no committed lockfile yet; it joins the audit "
                "once the refresh creates and commits it."
            )
        lines.append("")

    if unfixable:
        verdict = "FAIL: unfixable advisory present - CALL OWNER"
    elif fixable:
        verdict = "PASS with action required: UPDATE ASAP"
    else:
        verdict = "PASS: no vulnerabilities"
    lines.append(f"**Verdict: {verdict}**")
    return "\n".join(lines) + "\n"


def infrastructure_summary(phase: str, detail: str) -> str:
    """Build the fail-closed summary for an infrastructure error."""
    detail = detail.replace("\n", " ").strip()
    return (
        f"## Security supply chain audit report ({phase})\n"
        "\n"
        "> [!CAUTION]\n"
        "> AUDIT INFRASTRUCTURE FAILURE - fail closed.\n"
        f"> {_cell(detail)}\n"
        "\n"
        "A broken guard must never pass silently: the run fails so the\n"
        "scheduled-failure notification reaches the owner.\n"
        "\n"
        "**Verdict: FAIL: audit infrastructure failure**\n"
    )


def write_summary(markdown: str) -> None:
    """Append the report to the GitHub step summary when available."""
    path = os.environ.get("GITHUB_STEP_SUMMARY")
    if path:
        with open(path, "a", encoding="utf-8") as handle:
            handle.write(markdown)


def run(args: argparse.Namespace) -> int:
    """Full audit run; returns the process exit code."""
    repo_root = Path.cwd()
    fixable: list[dict] = []
    unfixable: list[dict] = []
    informational: list[dict] = []
    audited: list[str] = []
    skipped: list[str] = []
    try:
        locks = discover_lockfiles(repo_root)
        if args.json_file:
            audited.append(str(args.json_file))
            report = parse_report(
                Path(args.json_file).read_text(encoding="utf-8"),
                context=str(args.json_file),
            )
            f, u, w = classify(report)
            label = Path(args.json_file)
            fixable.extend(finding_row(i, label) for i in f)
            unfixable.extend(finding_row(i, label) for i in u)
            informational.extend(finding_row(i, label) for i in w)
        else:
            for lock in locks:
                if not lock.exists():
                    skipped.append(str(lock))
                    continue
                audited.append(str(lock))
                f, u, w = classify(run_audit(repo_root, lock))
                fixable.extend(finding_row(i, lock) for i in f)
                unfixable.extend(finding_row(i, lock) for i in u)
                informational.extend(finding_row(i, lock) for i in w)
    except GuardError as exc:
        detail = str(exc)
        print(f"::error::[security-guard] {detail}", file=sys.stderr)
        write_summary(infrastructure_summary(args.phase, detail))
        print("[security-guard] verdict=FAIL reason=infrastructure")
        return 1

    fixable.sort(key=lambda r: (r["rank"], r["id"], r["lockfile"]))
    unfixable.sort(key=lambda r: (r["rank"], r["id"], r["lockfile"]))
    for row in unfixable:
        print(
            f"::error::CALL OWNER: {row['id']} on {row['package']} in "
            f"{row['lockfile']}: no patched version exists, a routine "
            "update cannot fix it"
        )
    for row in fixable:
        print(
            f"::warning::UPDATE ASAP: {row['id']} on {row['package']} in "
            f"{row['lockfile']}: fixed in {row['patched']}"
        )
    for path in skipped:
        print(
            f"::warning::No committed lockfile for {path}; skipped from "
            "this audit until the refresh commits one"
        )
    print(
        f"[security-guard] phase={args.phase} audited={len(audited)} "
        f"skipped={len(skipped)} tier1={len(fixable)} "
        f"tier2={len(unfixable)} info={len(informational)}"
    )
    for row in unfixable:
        print(
            f"[security-guard] tier2 {row['id']} package={row['package']} "
            f"severity={row['severity']} lock={row['lockfile']} fixed=none"
        )
    for row in fixable:
        print(
            f"[security-guard] tier1 {row['id']} package={row['package']} "
            f"severity={row['severity']} lock={row['lockfile']} "
            f"fixed={row['patched']}"
        )
    for row in informational:
        print(
            f"[security-guard] info {row.get('id') or 'unknown'} "
            f"package={row['package']} note={row['note']} "
            f"lock={row['lockfile']}"
        )
    verdict = "FAIL" if unfixable else "PASS"
    print(f"[security-guard] verdict={verdict}")
    write_summary(
        render_report(args.phase, audited, skipped, fixable, unfixable, informational)
    )
    return 1 if unfixable else 0


def make_advisory(**overrides: object) -> dict:
    """Build one advisory object for the self-test fixtures."""
    base: dict = {
        "id": "RUSTSEC-2026-0001",
        "package": "alpha",
        "title": "Example advisory",
        "severity": "high",
        "aliases": ["CVE-2026-0001"],
        "url": "https://rustsec.org/advisories/RUSTSEC-2026-0001.html",
        "patched": [],
        "unaffected": [],
    }
    base.update(overrides)
    return base


def make_vuln(**overrides: object) -> dict:
    """Build one vulnerabilities.list item for the fixtures."""
    advisory = make_advisory(
        **{k: v for k, v in overrides.items() if k in make_advisory()}
    )
    versions = overrides.get("versions", {"patched": [], "unaffected": []})
    return {"advisory": advisory, "versions": versions}


def make_report(vulns: list[dict], warnings: list[dict] | None = None) -> dict:
    """Wrap findings into a cargo audit JSON report skeleton."""
    return {
        "results": {
            "list": [
                {
                    "vulnerabilities": {"count": len(vulns), "list": vulns},
                    "warnings": {"list": warnings or []},
                }
            ]
        }
    }


def self_test() -> int:
    """Fixture-driven tests for the classifier, parser and rendering."""
    checks: list[tuple[str, bool]] = []

    def check(name: str, condition: bool) -> None:
        checks.append((name, bool(condition)))

    clean = make_report([])
    f, u, w = classify(clean)
    check("clean report classifies empty", (f, u, w) == ([], [], []))

    fixable_report = make_report(
        [make_vuln(patched=["^1.2.3"], id="RUSTSEC-2026-0001")]
    )
    f, u, w = classify(fixable_report)
    check("fixable finding lands in tier 1", len(f) == 1 and not u)

    unfixable_report = make_report(
        [make_vuln(patched=[], id="RUSTSEC-2026-0002", package="beta")]
    )
    f, u, w = classify(unfixable_report)
    check("patched-less finding lands in tier 2", len(u) == 1 and not f)
    check("tier 2 drives exit 1", (1 if u else 0) == 1)

    mixed = make_report(
        [
            make_vuln(patched=["^1.2.3"], id="RUSTSEC-2026-0003"),
            make_vuln(patched=[], id="RUSTSEC-2026-0004", package="beta"),
        ],
        warnings=[
            {
                "advisory": make_advisory(
                    id="RUSTSEC-2026-0005",
                    package="gamma",
                    informational="unmaintained",
                ),
                "versions": {"patched": [], "unaffected": []},
            }
        ],
    )
    f, u, w = classify(mixed)
    check(
        "mixed report splits into three buckets",
        len(f) == 1 and len(u) == 1 and len(w) == 1,
    )

    legacy = {
        "advisory": {
            "id": "RUSTSEC-2026-0006",
            "package": "delta",
            "patched_versions": ">=1.2.3, <2.0.0",
        },
        "versions": {},
    }
    check(
        "legacy patched_versions string still counts as fixable",
        len(classify(make_report([legacy]))[0]) == 1,
    )

    sibling = {
        "advisory": make_advisory(id="RUSTSEC-2026-0007", package="epsilon"),
        "versions": {"patched": ["1.9.0"], "unaffected": []},
    }
    check(
        "sibling versions object still counts as fixable",
        len(classify(make_report([sibling]))[0]) == 1,
    )

    row = finding_row(
        make_vuln(patched=["1.0"], package="pi|pe"), Path("Cargo.lock")
    )
    table = md_table([dict(row, advisory=_advisory_cell(row))], [("Package", "package")])
    check("table cells escape pipes", "pi\\|pe" in table)
    check("advisory cell links the url", "rustsec.org" in _advisory_cell(row))

    noisy = 'warning line\n{"results": {"list": []}}\ntrailing'
    check(
        "parser tolerates log noise around the JSON",
        parse_report(noisy, "noise") == {"results": {"list": []}},
    )
    try:
        parse_report("not json at all", "garbage")
        raised = False
    except GuardError:
        raised = True
    check("malformed report raises GuardError", raised)

    report_text = render_report(
        "baseline",
        ["Cargo.lock"],
        [],
        [finding_row(make_vuln(patched=["^1.2.3"]), Path("Cargo.lock"))],
        [finding_row(make_vuln(patched=[], package="beta"), Path("Cargo.lock"))],
        [],
    )
    check(
        "report contains both banners",
        "CALL OWNER" in report_text and "UPDATE ASAP" in report_text,
    )
    check("report ends in a FAIL verdict", "Verdict: FAIL" in report_text)
    infra = infrastructure_summary("baseline", "boom\nboom")
    check(
        "infrastructure summary flattens detail newlines",
        "boom boom" in infra and "\nboom" not in infra,
    )

    passed = sum(1 for _, ok in checks if ok)
    for name, ok in checks:
        print(f"  {'PASS' if ok else 'FAIL'} {name}")
    print(f"self-test: {passed}/{len(checks)} checks passed")
    return 0 if passed == len(checks) else 1


def main(argv: list[str] | None = None) -> int:
    """Entry point; returns the process exit code."""
    parser = argparse.ArgumentParser(
        description="Tiered RustSec supply chain guard"
    )
    parser.add_argument(
        "--phase",
        default="audit",
        help="label baked into the report header (baseline / post-update)",
    )
    parser.add_argument(
        "--json-file",
        help="classify a saved cargo audit JSON report instead of running "
        "the tool (offline debugging)",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run the built-in fixture tests and exit",
    )
    args = parser.parse_args(argv)
    if args.self_test:
        return self_test()
    return run(args)


if __name__ == "__main__":
    sys.exit(main())
