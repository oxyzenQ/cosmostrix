#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
"""The supply-chain verdict (night-solid-defense-1) - AXP1-ZnQ
(Architect's eXecution Protocol 1).

The Sunday maintenance lane audits every workspace lockfile against
the RustSec advisory database and speaks the owner's two scenarios,
under the estate's sentinel identity - the same bot the zejtron
sibling runs (github.com/oxyzenQ/zejtron, the mature reference).

  scenario A (fixable): the sweep cleared the finding. The advisory
    was real, the auto-update took the clean version in the same
    run, and the validated lane ships that fix. The job stays green
    and the owner still reads a loud notice: without the Sunday lane
    the tree would have carried the advisory for a week.

  scenario B (unfixable): the finding survives the sweep - no fixed
    version inside the current version requirements, or no fix
    released at all. The job fails hard: the dep is dangerous and
    no automatic update can take it - CALL OWNER. The commit job is
    gated on the verdict, so nothing ships while the lane is red.

Hard kinds (a verdict failure when they survive the sweep):
vulnerability, unmaintained, unsound, yanked. Informational
advisories (notice) never gate. The yanked kind keys on the exact
version: the identity of a yanked release is that release.

Fail-closed law: a scanner that cannot run or cannot be parsed is a
verdict failure, never a silent pass. A missing tool, a missing
lockfile set, a malformed report or any cargo audit exit outside
{0, 1} fails the lane.

Identity: the banners, the annotations, and the step summary all
sign as AXP1-ZnQ (Architect's eXecution Protocol 1), so a red
Sunday names its watcher.

Modes:
  --pre   scan the as-committed lockfiles, store findings for the
          post-pass diff
  --post  scan the swept lockfiles, diff against the stored pre
          findings, emit the verdict (exit 1 on scenario B)
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

# cargo audit documented exit codes: 0 = clean, 1 = findings exist
# (the JSON report is still authoritative), anything else is an
# infrastructure error. Only 0 and 1 may be read as a usable report.
CARGO_AUDIT_OK = 0
CARGO_AUDIT_FINDINGS = 1

WALK_SCRIPT = "scripts/ci/update-all-workspaces.sh"

HARD_KINDS = ("vulnerability", "unmaintained", "unsound", "yanked")
INFO_KIND = "informational"

SEVERITY_RANK = {
    "critical": 0,
    "high": 1,
    "medium": 2,
    "low": 3,
    "unspecified": 4,
}

STATE_DIRNAME = "supplychain-verdict"
PRE_TSV = "pre.tsv"
POST_TSV = "post.tsv"
CLEARED_TSV = "cleared.tsv"
PRE_JSON_DIR = "pre"
POST_JSON_DIR = "post"

# One finding per line: workspace, kind, id, package, version,
# severity, patched, title, url. The diff keys drop the version
# (an update fixes BY moving the version) except for yanked, whose
# identity is the exact release.
TSV_COLUMNS = (
    "workspace",
    "kind",
    "id",
    "package",
    "version",
    "severity",
    "patched",
    "title",
    "url",
)


class VerdictError(Exception):
    """Audit infrastructure failure; the verdict fails closed on it."""


def state_dir() -> Path:
    """Per-run scratch shared by the pre-pass and the verdict."""
    return Path(os.environ.get("RUNNER_TEMP", "/tmp")) / STATE_DIRNAME


def discover_lockfiles(repo_root: Path) -> list[Path]:
    """Derive every workspace lockfile from the canonical manifest walk.

    The walk is the single source of truth for manifest discovery: the
    refresh phase consumes its default mode and the commit phase its
    --list mode, so the verdict's scope can never drift from either.
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
        raise VerdictError(f"manifest walk failed (exit {proc.returncode}): {detail}")
    locks: list[Path] = []
    for line in proc.stdout.splitlines():
        manifest = line.strip()
        if not manifest:
            continue
        if not manifest.endswith("Cargo.toml"):
            raise VerdictError(f"walk emitted a non-manifest path: {manifest}")
        locks.append(Path(manifest[: -len("Cargo.toml")] + "Cargo.lock"))
    if not locks:
        raise VerdictError("manifest walk found no Cargo.toml")
    return locks


def workspace_name(lockfile: Path) -> str:
    """The workspace label: root for the repo root, else its directory."""
    parent = lockfile.parent
    if not str(parent) or str(parent) == ".":
        return "root"
    return str(parent)


def parse_report(text: str, context: str) -> dict:
    """Parse a cargo audit JSON report, tolerating noise around it."""
    start = text.find("{")
    end = text.rfind("}")
    if start == -1 or end <= start:
        raise VerdictError(f"no JSON report found for {context}")
    try:
        return json.loads(text[start : end + 1])
    except json.JSONDecodeError as exc:
        raise VerdictError(f"malformed JSON report for {context}: {exc}") from exc


def run_audit(repo_root: Path, lockfile: Path, json_out: Path) -> dict:
    """Run cargo audit against one lockfile and return its JSON report.

    The raw report is kept beside the state so a red Sunday can be
    post-mortemed without re-running the scanner.
    """
    proc = subprocess.run(
        ["cargo", "audit", "--json", "--file", str(lockfile)],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode not in (CARGO_AUDIT_OK, CARGO_AUDIT_FINDINGS):
        detail = (proc.stderr.strip() or "no stderr output")[-500:]
        raise VerdictError(
            f"cargo audit failed on {lockfile} (exit {proc.returncode}): {detail}"
        )
    report = parse_report(proc.stdout, context=str(lockfile))
    json_out.parent.mkdir(parents=True, exist_ok=True)
    json_out.write_text(json.dumps(report, indent=2), encoding="utf-8")
    return report


def _as_dict(value: object) -> dict:
    return value if isinstance(value, dict) else {}


def _patched_list(item: dict) -> list[str]:
    """Fixed-version requirements for one finding, across schema shapes.

    cargo audit 0.21+ serializes the patched list at the advisory
    level; older releases carry it in the sibling versions object,
    and very old ones used a single patched_versions string. Schema
    drift across releases must never flip a scenario, so all three
    shapes are accepted.
    """
    advisory = _as_dict(item.get("advisory"))
    value = advisory.get("patched")
    if isinstance(value, list) and value:
        return [str(v) for v in value]
    versions = _as_dict(item.get("versions"))
    value = versions.get("patched")
    if isinstance(value, list) and value:
        return [str(v) for v in value]
    value = advisory.get("patched_versions")
    if isinstance(value, str) and value.strip():
        return [value.strip()]
    return []


def _normalize_kind(raw: object) -> str:
    """Map a warning category onto the verdict's kind vocabulary."""
    kind = str(raw or "").strip().lower()
    if kind in HARD_KINDS or kind == "vulnerability":
        return kind
    return INFO_KIND


def make_row(workspace: str, kind: str, item: dict) -> dict:
    """Flatten one cargo audit finding into a verdict row."""
    advisory = _as_dict(item.get("advisory"))
    package_obj = _as_dict(item.get("package"))
    package = str(package_obj.get("name") or advisory.get("package") or "unknown")
    version = str(package_obj.get("version") or "?")
    severity = str(advisory.get("severity") or "unspecified")
    aliases = [str(a) for a in advisory.get("aliases") or []]
    return {
        "workspace": workspace,
        "kind": kind,
        "id": str(advisory.get("id") or "unknown"),
        "package": package,
        "version": version,
        "severity": severity,
        "rank": SEVERITY_RANK.get(severity.lower(), 4),
        "patched": ", ".join(_patched_list(item)) or "",
        "aliases": ", ".join(aliases),
        "title": str(advisory.get("title") or "").strip(),
        "url": str(advisory.get("url") or "").strip(),
    }


def _rows_from_group(group: object, workspace: str, kind: str) -> list[dict]:
    """Extract rows from one findings group in either container shape."""
    rows: list[dict] = []
    if isinstance(group, dict):
        items = group.get("list") or []
    elif isinstance(group, list):
        items = group
    else:
        items = []
    for item in items:
        rows.append(make_row(workspace, kind, _as_dict(item)))
    return rows


def findings_from_report(report: dict, workspace: str) -> list[dict]:
    """Every finding of one report, classified into verdict kinds.

    Two report shapes are accepted, both proven against the real
    scanner: the pinned 0.22.x shape (flat top-level `vulnerabilities`
    and keyed `warnings` groups - the shape the real 0.22.2 binary
    emits, probed with live RUSTSEC advisories) and the pre-0.22
    `results.list` shape. The warnings containers appear as a flat
    list with a per-item kind or as keyed groups
    (unmaintained / unsound / yanked / notice). An unknown warning
    kind degrades to informational - the hard set is a fixed
    vocabulary, never a guess.
    """
    rows: list[dict] = []
    # The pinned 0.22.x shape: flat top-level containers, one per
    # report (the scanner emits exactly one per --file invocation).
    rows.extend(
        _rows_from_group(report.get("vulnerabilities"), workspace, "vulnerability")
    )
    warnings = report.get("warnings") or {}
    if isinstance(warnings, dict):
        for item in warnings.get("list") or []:
            advisory = _as_dict(_as_dict(item).get("advisory"))
            raw_kind = (
                item.get("kind")
                or item.get("category")
                or advisory.get("informational")
            )
            rows.append(make_row(workspace, _normalize_kind(raw_kind), _as_dict(item)))
        for key in ("unmaintained", "unsound", "yanked"):
            rows.extend(_rows_from_group(warnings.get(key), workspace, key))
        rows.extend(_rows_from_group(warnings.get("notice"), workspace, INFO_KIND))
    rows.extend(_rows_from_group(report.get("informational"), workspace, INFO_KIND))
    # The pre-0.22 shape: results.list, one block per scanned lockfile.
    for result in (report.get("results") or {}).get("list") or []:
        rows.extend(
            _rows_from_group(
                (result.get("vulnerabilities") or {}), workspace, "vulnerability"
            )
        )
        result_warnings = result.get("warnings") or {}
        for item in (
            result_warnings.get("list") or []
            if isinstance(result_warnings, dict)
            else []
        ):
            advisory = _as_dict(_as_dict(item).get("advisory"))
            raw_kind = (
                item.get("kind")
                or item.get("category")
                or advisory.get("informational")
            )
            rows.append(make_row(workspace, _normalize_kind(raw_kind), _as_dict(item)))
        for key in ("unmaintained", "unsound", "yanked"):
            rows.extend(_rows_from_group(result_warnings.get(key), workspace, key))
        rows.extend(_rows_from_group(result.get("informational"), workspace, INFO_KIND))
    unique: dict[tuple, dict] = {}
    for row in rows:
        unique[
            (row["workspace"], row["kind"], row["id"], row["package"], row["version"])
        ] = row
    return sorted(unique.values(), key=lambda r: (r["kind"], r["rank"], r["id"]))


def finding_key(row: dict) -> tuple:
    """Diff identity: the version moves when an update fixes, except
    for yanked, whose identity is the exact release."""
    if row["kind"] == "yanked":
        return (
            row["workspace"],
            row["kind"],
            row["id"],
            row["package"],
            row["version"],
        )
    return (row["workspace"], row["kind"], row["id"], row["package"])


def _clean_cell(value: object) -> str:
    """TSV-safe text: no tabs, no newlines, never empty."""
    text = (
        str(value if value is not None else "")
        .replace("\t", " ")
        .replace("\n", " ")
        .strip()
    )
    return text


def rows_to_tsv(rows: list[dict]) -> str:
    lines = ["\t".join(TSV_COLUMNS)]
    for row in rows:
        lines.append("\t".join(_clean_cell(row[col]) for col in TSV_COLUMNS))
    return "\n".join(lines) + "\n"


def tsv_to_rows(text: str) -> list[dict]:
    rows: list[dict] = []
    for line in text.splitlines()[1:]:
        if not line.strip():
            continue
        parts = line.split("\t")
        parts += [""] * (len(TSV_COLUMNS) - len(parts))
        row = dict(zip(TSV_COLUMNS, parts))
        row["rank"] = SEVERITY_RANK.get(row["severity"].lower(), 4)
        rows.append(row)
    return rows


def write_state(name: str, content: str) -> Path:
    path = state_dir() / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    return path


def read_state(name: str) -> str | None:
    path = state_dir() / name
    if not path.exists():
        return None
    return path.read_text(encoding="utf-8")


def _cell(value: object) -> str:
    """Render one markdown table cell; escape pipes, keep empties."""
    text = str(value).replace("|", "\\|")
    return text if text else "-"


def _advisory_cell(row: dict) -> str:
    if row.get("url"):
        return f"[{_cell(row['id'])}]({_cell(row['url'])})"
    return _cell(row["id"])


def md_table(rows: list[dict], verdict_label: str) -> str:
    """The mature verdict table: one row per finding, verdict signed."""
    lines = [
        "| workspace | kind | id | package | version | severity | patched | verdict |",
        "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for row in sorted(rows, key=lambda r: (r["rank"], r["id"])):
        display = dict(row, id=_advisory_cell(row))
        lines.append(
            "| {workspace} | {kind} | {id} | {package} | {version} | {severity} | {patched} | {verdict} |".format(
                workspace=_cell(display["workspace"]),
                kind=_cell(display["kind"]),
                id=display["id"],
                package=_cell(display["package"]),
                version=_cell(display["version"]),
                severity=_cell(display["severity"]),
                patched=_cell(display["patched"] or "none"),
                verdict=_cell(verdict_label),
            )
        )
    return "\n".join(lines)


def render_summary(
    ws_count: int,
    pre_count: int,
    cleared: list[dict],
    still_bad: list[dict],
    informational: list[dict],
) -> str:
    """The step summary: AXP1-ZnQ (Architect's eXecution Protocol 1)
    signed, mature table shape."""
    lines = [
        "### AXP1-ZnQ (Architect's eXecution Protocol 1) supply-chain "
        "verdict (night-solid-defense-1)",
        "",
        (
            f"workspaces scanned: {ws_count} | pre-sweep findings: {pre_count} | "
            f"cleared by the sweep: {len(cleared)} | still present (hard): {len(still_bad)} | "
            f"informational: {len(informational)}"
        ),
    ]
    if cleared:
        lines += ["", md_table(cleared, "cleared - fix ships in this run")]
    if still_bad:
        lines += ["", md_table(still_bad, "STILL PRESENT - call owner")]
    if informational:
        lines += ["", md_table(informational, "notice")]
    return "\n".join(lines) + "\n"


def write_summary(markdown: str) -> None:
    """Append the verdict to the GitHub step summary when available."""
    path = os.environ.get("GITHUB_STEP_SUMMARY")
    if path:
        with open(path, "a", encoding="utf-8") as handle:
            handle.write(markdown)


def scan_all(
    repo_root: Path, locks: list[Path], json_dirname: str
) -> tuple[list[dict], list[str]]:
    """Scan every lockfile; returns (rows, skipped workspace notes).

    A manifest whose lockfile does not exist yet is skipped with a
    note in the pre-pass (a committed lane carries nothing to
    snapshot there); in the post pass the refresh has run cargo
    update everywhere, so a missing lockfile is an infrastructure
    failure and fails closed.
    """
    rows: list[dict] = []
    skipped: list[str] = []
    for lock in locks:
        ws = workspace_name(lock)
        if not lock.exists():
            skipped.append(ws)
            continue
        report = run_audit(repo_root, lock, state_dir() / json_dirname / f"{ws}.json")
        rows.extend(findings_from_report(report, ws))
    return rows, skipped


def run_pre(repo_root: Path) -> int:
    """Scan the as-committed lane and store the snapshot. Gates nothing."""
    locks = discover_lockfiles(repo_root)
    try:
        rows, skipped = scan_all(repo_root, locks, PRE_JSON_DIR)
    except VerdictError as exc:
        _fail_closed(str(exc))
        return 1
    write_state(PRE_TSV, rows_to_tsv(rows))
    for ws in skipped:
        print(
            f"::warning title=AXP1-ZnQ notice::{ws} has no committed lockfile yet - skipped from the pre-pass snapshot"
        )
    print(
        f"AXP1-ZnQ pre-pass: {len(rows)} finding(s) stored across "
        f"{len(locks) - len(skipped)} workspace(s)"
    )
    return 0


def _fail_closed(detail: str) -> None:
    detail = detail.replace("\n", " ").strip()
    print(f"::error title=AXP1-ZnQ fail-closed::{detail}", file=sys.stderr)
    write_summary(
        "### AXP1-ZnQ (Architect's eXecution Protocol 1) supply-chain "
        "verdict (night-solid-defense-1)\n"
        "\n"
        "AUDIT INFRASTRUCTURE FAILURE - fail closed: a scanner that\n"
        f"cannot run is a verdict failure, never a silent pass.\n"
        f"Detail: {_cell(detail)}\n"
        "\n"
        "**AXP1-ZnQ verdict: FAIL - audit infrastructure failure**\n"
    )
    print("AXP1-ZnQ verdict: FAIL - audit infrastructure failure (fail closed)")


def survival_fix(row: dict) -> str:
    """Why the sweep cannot take this finding - shown to the owner.

    A patched version that cargo update cannot reach means a manual
    version-bump decision; no patched version at all means there is
    no update path. Both are scenario B: CALL OWNER.
    """
    if row["patched"]:
        return (
            f"fixed in {row['patched']} - outside the current "
            "version requirement, cargo update cannot take it"
        )
    return "no fixed version released - no automatic update path exists"


def run_post(repo_root: Path) -> int:
    """Scan the swept lane, diff against the snapshot, emit the verdict."""
    locks = discover_lockfiles(repo_root)
    try:
        post_rows, skipped = scan_all(repo_root, locks, POST_JSON_DIR)
    except VerdictError as exc:
        _fail_closed(str(exc))
        return 1
    pre_text = read_state(PRE_TSV)
    if pre_text is None:
        print(
            "::warning title=AXP1-ZnQ notice::no pre-pass snapshot found - "
            "judging the swept lane alone"
        )
        pre_rows: list[dict] = []
    else:
        pre_rows = tsv_to_rows(pre_text)

    post_keys = {finding_key(row) for row in post_rows}
    cleared = [row for row in pre_rows if finding_key(row) not in post_keys]
    still_bad = [row for row in post_rows if row["kind"] in HARD_KINDS]
    informational = [row for row in post_rows if row["kind"] == INFO_KIND]

    write_state(POST_TSV, rows_to_tsv(post_rows))
    write_state(CLEARED_TSV, rows_to_tsv(cleared))

    ws_count = len(locks) - len(skipped)
    write_summary(
        render_summary(ws_count, len(pre_rows), cleared, still_bad, informational)
    )

    if cleared:
        print("==================================================================")
        print("  AXP1-ZnQ: vulnerable dependency cleared by this run")
        print("==================================================================")
        for row in cleared:
            print(
                f"  - {row['workspace']} | {row['kind']} {row['id']} | "
                f"{row['package']} {row['version']} | severity {row['severity']}"
            )
            print("    the Sunday sweep took the fix; without this lane the")
            print("    tree would have carried the advisory for a week")
            print(
                f"::warning title=AXP1-ZnQ notice::{row['workspace']} "
                f"{row['kind']} {row['id']} ({row['package']} {row['version']}) "
                "was present and the sweep cleared it - the fix ships in "
                "this run; without the weekly lane the tree would have "
                "carried it"
            )
        print("==================================================================")

    if still_bad:
        print("==================================================================")
        print("  AXP1-ZnQ SUPPLY-CHAIN VERDICT: FAIL - CALL OWNER")
        print("==================================================================")
        for row in still_bad:
            fix = survival_fix(row)
            print(
                f"  - {row['workspace']} | {row['kind']} {row['id']} | "
                f"{row['package']} {row['version']} | severity {row['severity']}"
            )
            print(f"    {fix}")
            print(
                f"::error title=AXP1-ZnQ::{row['workspace']} {row['kind']} "
                f"{row['id']} ({row['package']} {row['version']}, severity "
                f"{row['severity']}) survives the sweep ({fix}) - dangerous "
                "dep, cannot be auto-updated; CALL OWNER"
            )
        print("==================================================================")
        print("  AXP1-ZNQ HOLDS THE LANE: the deps above are dangerous and")
        print("  cannot be auto-updated. CALL THE OWNER - the commit lane is")
        print("  gated on this verdict, nothing ships while the advisory")
        print("  survives the sweep.")
        print("==================================================================")
        print(
            f"still present: {len(still_bad)} hard finding(s), "
            f"{len(informational)} informational, {len(cleared)} cleared "
            "by the sweep"
        )
        return 1

    if informational:
        for row in informational:
            print(
                f"  notice: {row['workspace']} | {row['id']} | "
                f"{row['package']} {row['version']} (informational)"
            )
    print(
        f"AXP1-ZnQ verdict: PASS - {ws_count} workspace(s) scanned, "
        f"{len(still_bad)} hard finding(s), {len(cleared)} cleared by the "
        f"sweep, {len(informational)} informational"
    )
    return 0


def make_report(
    vulns: list[dict], warnings: object = None, informational: object = None
) -> dict:
    """Wrap findings into a cargo audit JSON report skeleton.

    The shape proven against the real 0.22.2 binary: flat top-level
    containers (vulnerabilities / warnings keyed groups / informational),
    no results wrapper.
    """
    result: dict = {"vulnerabilities": {"count": len(vulns), "list": vulns}}
    if warnings is not None:
        result["warnings"] = warnings
    if informational is not None:
        result["informational"] = informational
    return result


def make_legacy_report(
    vulns: list[dict], warnings: object = None, informational: object = None
) -> dict:
    """The pre-0.22 results.list shape, kept for schema-drift parity."""
    result: dict = {
        "vulnerabilities": {"count": len(vulns), "list": vulns},
    }
    if warnings is not None:
        result["warnings"] = warnings
    if informational is not None:
        result["informational"] = informational
    return {"results": {"list": [result]}}


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


def make_finding(**overrides: object) -> dict:
    """Build one findings-list item with the 0.21+ package shape."""
    advisory = make_advisory(
        **{k: v for k, v in overrides.items() if k in make_advisory()}
    )
    item = {
        "advisory": advisory,
        "package": {
            "name": overrides.get("package", advisory["package"]),
            "version": overrides.get("version", "1.0.0"),
        },
        "versions": {"patched": [], "unaffected": []},
    }
    return item


def self_test() -> int:
    """Fixture-driven tests: extraction, keying, diff, verdict."""
    checks: list[tuple[str, bool]] = []

    def check(name: str, condition: bool) -> None:
        checks.append((name, bool(condition)))

    # Extraction: the package object carries the locked version.
    report = make_report(
        [make_finding(id="RUSTSEC-2026-0001", package="alpha", version="1.2.3")]
    )
    rows = findings_from_report(report, "root")
    check(
        "vulnerability row keeps the locked version",
        len(rows) == 1
        and rows[0]["kind"] == "vulnerability"
        and rows[0]["package"] == "alpha"
        and rows[0]["version"] == "1.2.3",
    )

    # The proven 0.22.2 shapes, replayed from a live probe: the
    # arrayref 0.3.10 malicious advisory carries no patched version,
    # and the instant unmaintained warning rides a keyed warnings
    # group with a per-item kind.
    probe = make_report(
        [make_finding(id="RUSTSEC-2026-0260", package="arrayref", version="0.3.10")],
        warnings={
            "unmaintained": [
                {
                    "kind": "unmaintained",
                    "advisory": make_advisory(
                        id="RUSTSEC-2024-0384", package="instant"
                    ),
                    "package": {"name": "instant", "version": "0.1.13"},
                }
            ]
        },
    )
    rows = findings_from_report(probe, "root")
    check(
        "probe-replayed 0.22.2 shape yields the exact hard rows",
        len(rows) == 2
        and rows[0]["kind"] == "unmaintained"
        and rows[1]["kind"] == "vulnerability"
        and rows[1]["version"] == "0.3.10",
    )

    # The pre-0.22 results.list shape still extracts (schema drift).
    legacy_rows = findings_from_report(
        make_legacy_report([make_finding(package="legacy", version="1.0.0")]), "root"
    )
    check(
        "pre-0.22 results.list shape still extracts",
        len(legacy_rows) == 1 and legacy_rows[0]["package"] == "legacy",
    )

    # Warnings flat-list shape: a per-item kind drives the vocabulary.
    flat = make_report(
        [],
        warnings={
            "list": [
                {
                    "advisory": make_advisory(id="RUSTSEC-2026-0002", package="beta"),
                    "kind": "unmaintained",
                    "package": {"name": "beta", "version": "0.9.0"},
                }
            ]
        },
    )
    rows = findings_from_report(flat, "fuzz")
    check(
        "flat warning kind classifies unmaintained as hard",
        len(rows) == 1 and rows[0]["kind"] == "unmaintained",
    )

    # Keyed-group shape: warnings.yanked / unsound groups.
    keyed = make_report(
        [],
        warnings={
            "yanked": [
                {
                    "advisory": make_advisory(id="RUSTSEC-2026-0003", package="gamma"),
                    "package": {"name": "gamma", "version": "0.3.10"},
                }
            ]
        },
    )
    rows = findings_from_report(keyed, "root")
    check(
        "keyed yanked group classifies as hard yanked kind",
        len(rows) == 1
        and rows[0]["kind"] == "yanked"
        and rows[0]["version"] == "0.3.10",
    )

    # Informational notice never lands in the hard set.
    notice = make_report(
        [],
        warnings={
            "list": [
                {
                    "advisory": make_advisory(
                        id="RUSTSEC-2026-0004",
                        package="delta",
                        informational="notice",
                    ),
                    "package": {"name": "delta", "version": "2.0.0"},
                }
            ]
        },
    )
    rows = findings_from_report(notice, "root")
    check(
        "notice degrades to informational",
        len(rows) == 1 and rows[0]["kind"] == INFO_KIND,
    )

    # Keying: a vulnerability is cleared when the version moves; a
    # yanked finding keys on the exact release.
    vuln_pre = make_row(
        "root", "vulnerability", make_finding(package="alpha", version="1.2.3")
    )
    vuln_post = make_row(
        "root", "vulnerability", make_finding(package="alpha", version="1.4.0")
    )
    yank_pre = make_row(
        "root", "yanked", make_finding(package="gamma", version="0.3.10")
    )
    yank_post = make_row(
        "root", "yanked", make_finding(package="gamma", version="0.3.11")
    )
    check(
        "vulnerability key drops the version",
        finding_key(vuln_pre) == finding_key(vuln_post),
    )
    check(
        "yanked key keeps the exact version",
        finding_key(yank_pre) != finding_key(yank_post),
    )

    # The diff: the sweep takes the alpha fix, so the advisory
    # vanishes from the swept lane (scenario A, cleared); gamma moves
    # onto another yanked release and stays hard (scenario B).
    pre_rows = [vuln_pre, yank_pre]
    post_rows = [yank_post]
    post_keys = {finding_key(r) for r in post_rows}
    cleared = [r for r in pre_rows if finding_key(r) not in post_keys]
    still_bad = [r for r in post_rows if r["kind"] in HARD_KINDS]
    check(
        "sweep-cleared findings become scenario A",
        len(cleared) == 2
        and any(r["package"] == "alpha" for r in cleared)
        and any(r["kind"] == "yanked" for r in cleared),
    )
    check(
        "surviving yanked finding stays hard (scenario B)",
        len(still_bad) == 1 and still_bad[0]["kind"] == "yanked",
    )
    check(
        "scenario B drives exit 1",
        (1 if still_bad else 0) == 1,
    )

    # Verdict messages name the exact nature of the block.
    fixed_row = make_row("root", "vulnerability", make_finding(patched=["^2.0.0"]))
    unfixed_row = make_row("root", "vulnerability", make_finding(patched=[]))
    check(
        "surviving fixable advisory reports the version-requirement block",
        "outside the current version requirement" in survival_fix(fixed_row),
    )
    check(
        "surviving unfixed advisory reports no update path",
        "no automatic update path exists" in survival_fix(unfixed_row),
    )

    # TSV round-trip keeps the diff stable across processes.
    round_trip = tsv_to_rows(rows_to_tsv(pre_rows))
    check(
        "TSV round-trip preserves keys",
        {finding_key(r) for r in round_trip} == {finding_key(r) for r in pre_rows},
    )

    # Parser noise tolerance and fail-closed on garbage.
    noisy = 'warning line\n{"results": {"list": []}}\ntrailing'
    check(
        "parser tolerates log noise around the JSON",
        parse_report(noisy, "noise") == {"results": {"list": []}},
    )
    try:
        parse_report("not json at all", "garbage")
        raised = False
    except VerdictError:
        raised = True
    check("malformed report raises VerdictError", raised)

    # The summary carries the sentinel identity and the counts line.
    summary = render_summary(3, 2, [vuln_pre], [yank_pre], [])
    check(
        "summary signs as AXP1-ZnQ with the task tag",
        "AXP1-ZnQ (Architect's eXecution Protocol 1) supply-chain "
        "verdict (night-solid-defense-1)" in summary,
    )
    check(
        "summary counts line holds all four numbers",
        "cleared by the sweep: 1" in summary and "still present (hard): 1" in summary,
    )
    table = md_table(
        [dict(vuln_pre, id=_advisory_cell(vuln_pre))], "STILL PRESENT - call owner"
    )
    check(
        "verdict table carries the verdict column",
        "STILL PRESENT - call owner" in table and "rustsec.org" in table,
    )

    passed = sum(1 for _, ok in checks if ok)
    for name, ok in checks:
        print(f"  {'PASS' if ok else 'FAIL'} {name}")
    print(f"self-test: {passed}/{len(checks)} checks passed")
    return 0 if passed == len(checks) else 1


def main(argv: list[str] | None = None) -> int:
    """Entry point; returns the process exit code."""
    parser = argparse.ArgumentParser(
        description=(
            "AXP1-ZnQ (Architect's eXecution Protocol 1) "
            "supply-chain verdict (night-solid-defense-1)"
        )
    )
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument(
        "--pre",
        action="store_true",
        help="scan the as-committed lockfiles and store the snapshot",
    )
    mode.add_argument(
        "--post",
        action="store_true",
        help="scan the swept lockfiles and emit the verdict",
    )
    mode.add_argument(
        "--self-test",
        action="store_true",
        help="run the built-in fixture tests and exit",
    )
    args = parser.parse_args(argv)
    repo_root = Path.cwd()
    if args.self_test:
        return self_test()
    if args.pre:
        return run_pre(repo_root)
    return run_post(repo_root)


if __name__ == "__main__":
    sys.exit(main())
