#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""Resolve the latest STABLE Android NDK version for CI.

Owner policy (2026-08-30): zero hardcoded dependency versions in
.github/* -- everything resolves latest at run time. nttld/setup-ndk
closes that gap for the NDK in a way it cannot do itself:

- it has NO "latest" support: ndk-version: latest is spliced literally
  into the download URL (android-ndk-latest-linux-x86_64.zip) and 404s
  -- this exact breakage turned the android-aarch64 CI job red on three
  consecutive commits before it was noticed;
- it only accepts r-style names (r29, r27d) because Google's archive
  names are r-style -- the repository manifest's numeric versions
  (ndk;29.0.14206865) do not map to real archive URLs.

This script resolves the gap:

1. Fetch Google's official SDK repository manifest (repository2-3.xml,
   the same source sdkmanager reads).
2. Keep NDK packages on the STABLE channel (channel-0) whose Linux
   archive is a final release -- beta/rc/canary/preview archives are
   excluded even when Google lists them on the stable channel (e.g.
   android-ndk-r30-beta3-linux.zip).
3. Pick the highest revision and print its r-style name (e.g. r29),
   extracted from the archive URL.

Usage (CI):
    echo "ndk=$(python3 scripts/resolve-latest-ndk.py)" >> "$GITHUB_OUTPUT"

Fails loudly (exit 1) when nothing resolves -- never silently falls
back to a pinned version. The only numeric constant below, the r23
floor, is an archive-naming-era guard (NDK < r23 shipped arch-suffixed
archives and is 2021-era; building current Rust against it is doomed),
not a version pin.
"""

import re
import sys
import urllib.request

REPOSITORY_XML = "https://dl.google.com/android/repository/repository2-3.xml"
FETCH_RETRIES = 5
FETCH_TIMEOUT_S = 60
MIN_MAJOR = 23
PREVIEW_MARKERS = ("beta", "rc", "canary", "preview")


def fetch_manifest() -> str:
    """Fetch the repository manifest with retries; exit 1 on total failure."""
    last_error: Exception | None = None
    for attempt in range(1, FETCH_RETRIES + 1):
        try:
            with urllib.request.urlopen(
                REPOSITORY_XML, timeout=FETCH_TIMEOUT_S
            ) as response:
                return response.read().decode("utf-8", errors="replace")
        except OSError as error:
            last_error = error
            print(
                f"fetch attempt {attempt}/{FETCH_RETRIES} failed: {error}",
                file=sys.stderr,
            )
    raise SystemExit(f"error: could not fetch {REPOSITORY_XML}: {last_error}")


def linux_archive_url(package_body: str) -> str | None:
    """Return the linux <url> of a remotePackage body, if present."""
    for archive in re.finditer(r"<archive>(.*?)</archive>", package_body, re.DOTALL):
        chunk = archive.group(1)
        if "<host-os>linux</host-os>" not in chunk:
            continue
        url = re.search(r"<url>([^<]+)</url>", chunk)
        if url:
            return url.group(1)
    return None


def r_style_from_archive(archive: str) -> str:
    """android-ndk-r29-linux.zip -> r29 (the name setup-ndk expects)."""
    name = archive
    name = name.removeprefix("android-ndk-")
    name = name.removesuffix("-linux.zip")
    return name


def resolve_latest_stable_ndk(xml: str) -> tuple[tuple[int, ...], str]:
    """Return (revision_tuple, r_style) of the newest stable final NDK."""
    best: tuple[tuple[int, ...], str] | None = None
    candidates = 0
    for match in re.finditer(
        r'<remotePackage path="ndk;([0-9.]+)">(.*?)</remotePackage>', xml, re.DOTALL
    ):
        body = match.group(2)
        if '<channelRef ref="channel-0"/>' not in body:
            continue  # channel-0 = stable; 1=beta, 2=dev, 3=canary
        archive = linux_archive_url(body)
        if archive is None:
            continue
        lowered = archive.lower()
        if any(marker in lowered for marker in PREVIEW_MARKERS):
            continue  # preview archives (r30-beta3) are not boring-but-strong
        revision = re.search(
            r"<revision>\s*<major>(\d+)</major>\s*"
            r"<minor>(\d+)</minor>\s*<micro>(\d+)</micro>",
            body,
        )
        if revision is None:
            continue
        key = tuple(int(group) for group in revision.groups())
        if key[0] < MIN_MAJOR:
            continue  # archive-naming-era guard, not a version pin
        candidates += 1
        if best is None or key > best[0]:
            best = (key, r_style_from_archive(archive))
    if best is None:
        raise SystemExit(
            "error: no stable final linux NDK package found in the manifest "
            f"(checked {candidates} candidates) -- Google may have changed the "
            "manifest format; refusing to guess"
        )
    return best


def main() -> None:
    xml = fetch_manifest()
    key, r_style = resolve_latest_stable_ndk(xml)
    version = ".".join(str(part) for part in key)
    print(
        f"resolved latest stable NDK: {r_style} (revision {version})", file=sys.stderr
    )
    print(r_style)


if __name__ == "__main__":
    main()
