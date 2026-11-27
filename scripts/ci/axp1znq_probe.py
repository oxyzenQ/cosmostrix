#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# PLATFORM: UNIX-only (Linux, macOS, BSD). CI runs on Linux runners.
#
# COSMOSTRIX AXP1-ZnQ PROBE CORE - the shared reachability probe behind
# the repos-for-ci catalog (Architect's eXecution Protocol 1,
# night-learning-into-darkness-1)
#
"""The one probe both AXP1-ZnQ (Architect's eXecution Protocol 1)
commands ride (night-learning-into-darkness-1): the failover resolver
(scripts/ci/axp1znq_resolve.py) walks a catalog serially and stops at
the first live address, and the Sunday refresher
(scripts/ci/axp1znq_refresh.py) probes the whole catalog concurrently
and rewrites it around the survivors. One probe shape keeps the two
verdicts comparable - a mirror the resolver picks and a mirror the
refresher keeps are measured the same way, so the failover pick never
contradicts the weekly catalog.

The catalog law (repos-for-ci/*.txt):

  - one candidate address per line; the whole trimmed line IS the
    address, so a trailing comment on the same line would corrupt it -
    comments occupy their own line and start with '#';
  - a line with a scheme (https://old-releases.ubuntu.com/ubuntu) is
    an HTTP(S) address - probed with a real GET, TLS verified with the
    runner's own CA store (a cert failure is DOWN, never ignored);
  - a bare host:port (dns.example.com:53) is a TCP address - probed
    with a socket connect;
  - a bare hostname is accepted and defaults to https (defensive for
    an owner-authored line, never used by the shipped catalog);
  - directives are comment lines shaped `# key: value` (probe-path,
    max-age-days, purpose) and give the file its per-family probe and
    freshness policy without a central mapping table;
  - stdlib only (urllib, socket, concurrent.futures) - no third-party
    dependency joins the maintenance surface, the LTS law.

Freshness: the age of the probed artifact comes from its Last-Modified
header - the content's own modification time. The response's Date
header is the wrong witness (it is the response generation time, always
about now, so an age read off it never fires) - the Last-Modified read
is this port's freshness fix over the sibling estate's first sketch.
An artifact with no Last-Modified reports age None and is never
stale-flagged: absence of evidence is never fabricated evidence.

Every function here is thread-safe and allocation-quiet: the refresher
probes a whole catalog on a thread pool, so nothing in this module
touches shared state.
"""

from __future__ import annotations

import re
import socket
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from datetime import UTC, datetime
from email.utils import parsedate_to_datetime

VERDICT_PASS = "PASS"
VERDICT_STALE = "STALE"
VERDICT_DOWN = "DOWN"

DIRECTIVE_KEYS = ("purpose", "probe-path", "max-age-days")
DIRECTIVE_LINE = re.compile(r"^#\s*(" + "|".join(DIRECTIVE_KEYS) + r")\s*:\s*(.*?)\s*$")


@dataclass(frozen=True)
class CatalogDirectives:
    """The per-file policy a catalog's comment directives declare."""

    purpose: str = ""
    probe_path: str = ""
    max_age_days: int | None = 14  # None = freshness check disabled


@dataclass(frozen=True)
class CatalogEntry:
    """One active address line, parsed."""

    lineno: int
    raw: str
    kind: str  # 'http' or 'tcp'
    target: str  # the full URL, or host:port


@dataclass(frozen=True)
class ProbeResult:
    """The verdict for one catalog entry, shared by both commands."""

    verdict: str  # VERDICT_PASS / VERDICT_STALE / VERDICT_DOWN
    latency_ms: int | None  # the successful attempt's round trip
    age_days: int | None  # artifact age from Last-Modified, else None
    detail: str  # the human reason, written into dated catalog comments


def parse_directives(text: str) -> CatalogDirectives:
    """Read the `# key: value` policy lines out of a catalog body.

    Unknown comment lines are ignored (they are prose), and the last
    directive of a key wins so an owner edit cannot stack two policies.
    max-age-days accepts a positive integer or `off`/`none` to disable
    the freshness check for families whose artifact is legitimately
    frozen (an EOL-era dists index, a released suite's InRelease).
    """
    directives = CatalogDirectives()
    max_age_raw = ""
    for raw in text.splitlines():
        match = DIRECTIVE_LINE.match(raw.strip())
        if not match:
            continue
        key, value = match.group(1), match.group(2)
        if key == "purpose":
            directives = CatalogDirectives(
                purpose=value,
                probe_path=directives.probe_path,
                max_age_days=directives.max_age_days,
            )
        elif key == "probe-path":
            directives = CatalogDirectives(
                purpose=directives.purpose,
                probe_path=value,
                max_age_days=directives.max_age_days,
            )
        else:
            max_age_raw = value
    if max_age_raw:
        if max_age_raw.lower() in ("off", "none", "disabled"):
            directives = CatalogDirectives(
                purpose=directives.purpose,
                probe_path=directives.probe_path,
                max_age_days=None,
            )
        else:
            try:
                age = int(max_age_raw)
            except ValueError:
                age = 14  # malformed directive falls back to the default policy
            if age > 0:
                directives = CatalogDirectives(
                    purpose=directives.purpose,
                    probe_path=directives.probe_path,
                    max_age_days=age,
                )
    return directives


def parse_catalog(text: str) -> list[CatalogEntry]:
    """Parse a catalog body into its active address lines, in order."""
    entries: list[CatalogEntry] = []
    for lineno, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if "://" in line:
            entries.append(CatalogEntry(lineno, raw, "http", line))
        elif ":" in line:
            entries.append(CatalogEntry(lineno, raw, "tcp", line))
        else:
            entries.append(CatalogEntry(lineno, raw, "http", f"https://{line}"))
    return entries


def artifact_age_days(modified_header: str, now: datetime) -> int | None:
    """Age in whole days of the probed artifact from Last-Modified.

    The artifact a mirror serves carries its content's modification
    time in Last-Modified - the honest freshness witness (a mirror that
    answers fast but serves a months-old artifact is STALE, not PASS).
    Unparsable or missing headers return None and never fabricate an
    age; the response's Date header is deliberately not a fallback for
    a missing Last-Modified because it dates the response, not the
    content.
    """
    if not modified_header:
        return None
    cleaned = modified_header.strip()
    if cleaned.endswith(" UTC"):
        cleaned = cleaned[: -len(" UTC")] + " +0000"
    try:
        modified = parsedate_to_datetime(cleaned)
    except (TypeError, ValueError):
        return None
    if modified.tzinfo is None:
        modified = modified.replace(tzinfo=UTC)
    return max(0, int((now - modified).total_seconds() // 86400))


def probe_http(
    target: str,
    probe_path: str = "",
    connect_timeout: float = 4.0,
    total_timeout: float = 15.0,
    attempts: int = 2,
    max_age_days: int | None = None,
) -> ProbeResult:
    """Probe one HTTP(S) address with a real GET, TLS verified.

    The probe GETs the target (plus the probe path when the caller
    wants the family's real artifact, e.g. an HKP key lookup or a dists
    index - the base URL answering while the artifact 404s is DOWN, the
    lane cannot serve itself from a landing page). A 2xx verdict reads
    freshness off Last-Modified and hands the STALE call to the
    caller's policy.
    """
    url = target
    if probe_path:
        url = f"{target.rstrip('/')}/{probe_path.lstrip('/')}"
    last_detail = "no attempt made"
    for attempt in range(1, max(1, attempts) + 1):
        started = time.perf_counter()
        try:
            request = urllib.request.Request(
                url, method="GET", headers={"User-Agent": "AXP1-ZnQ-probe/1"}
            )
            with urllib.request.urlopen(request, timeout=total_timeout) as response:
                latency_ms = int((time.perf_counter() - started) * 1000)
                status = response.getcode()
                now = datetime.now(UTC)
                age = artifact_age_days(response.headers.get("Last-Modified", ""), now)
                if 200 <= status < 300:
                    if (
                        age is not None
                        and max_age_days is not None
                        and age > max_age_days
                    ):
                        return ProbeResult(
                            VERDICT_STALE,
                            latency_ms,
                            age,
                            f"stale artifact (age {age}d > {max_age_days}d)",
                        )
                    shape = f"HTTP {status}"
                    if age is not None:
                        shape += f", artifact age {age}d"
                    return ProbeResult(VERDICT_PASS, latency_ms, age, shape)
                return ProbeResult(
                    VERDICT_DOWN, latency_ms, age, f"HTTP {status} on the probe path"
                )
        except urllib.error.HTTPError as why:
            last_detail = f"HTTP {why.code} on the probe path"
        except (TimeoutError, urllib.error.URLError, OSError) as why:
            reason = getattr(why, "reason", why)
            last_detail = (
                f"unreachable ({reason})" if reason else f"unreachable ({why})"
            )
        except ValueError as why:
            return ProbeResult(VERDICT_DOWN, None, None, f"bad address ({why})")
        if attempt < attempts:
            time.sleep(1.0)
    return ProbeResult(
        VERDICT_DOWN, None, None, f"{last_detail} after {attempts} attempt(s)"
    )


def probe_tcp(
    target: str,
    connect_timeout: float = 4.0,
    attempts: int = 2,
) -> ProbeResult:
    """Probe one host:port address with a plain socket connect."""
    host, _, port_text = target.rpartition(":")
    if not host or not port_text.isdigit():
        return ProbeResult(VERDICT_DOWN, None, None, f"bad host:port ({target})")
    port = int(port_text)
    last_detail = "no attempt made"
    for attempt in range(1, max(1, attempts) + 1):
        started = time.perf_counter()
        try:
            with socket.create_connection((host, port), timeout=connect_timeout):
                latency_ms = int((time.perf_counter() - started) * 1000)
                return ProbeResult(
                    VERDICT_PASS, latency_ms, None, f"TCP {host}:{port} open"
                )
        except OSError as why:
            last_detail = f"no connection ({why})"
        if attempt < attempts:
            time.sleep(1.0)
    return ProbeResult(
        VERDICT_DOWN, None, None, f"{last_detail} after {attempts} attempt(s)"
    )


def probe_entry(
    entry: CatalogEntry,
    probe_path: str = "",
    connect_timeout: float = 4.0,
    total_timeout: float = 15.0,
    attempts: int = 2,
    max_age_days: int | None = None,
) -> ProbeResult:
    """Dispatch one catalog entry to the probe its kind wants."""
    if entry.kind == "tcp":
        return probe_tcp(
            entry.target, connect_timeout=connect_timeout, attempts=attempts
        )
    return probe_http(
        entry.target,
        probe_path=probe_path,
        connect_timeout=connect_timeout,
        total_timeout=total_timeout,
        attempts=attempts,
        max_age_days=max_age_days,
    )
