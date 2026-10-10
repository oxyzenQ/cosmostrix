# shellcheck shell=bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
#
#
# COSMOSTRIX GITHUB TOKEN RESOLUTION
#
# One implementation of "find a GitHub API token on this machine,
# without ever creating a new standing secret". The actions-pin
# health check (scripts/gates/check-actions-pins.sh) and the version
# sweep (scripts/ci/actions-version-sweep.sh) both read the GitHub
# API; a machine that pushes fine already holds a credential, and
# burning the anonymous 60/h ceiling mid-sweep turns live tags into
# false "unresolvable" verdicts (the zelynic NIGHT-improve-71 first
# apply caught that exact half-heal shape; the lib is the same
# one-implementation discipline carried into cosmostrix, night-
# improve-8). Resolution order, first hit wins:
#
#   1. GITHUB_TOKEN already in the environment (unchanged contract)
#   2. the gh CLI's own auth token ("gh auth token") - zero setup on
#      a gh-authenticated machine
#   3. git's own credential helper for https://github.com - the SAME
#      secret the machine already uses to push, read through
#      "git credential fill" with terminal prompts disabled and a
#      hard 5s timeout (a helper that would hang or pop a dialog is
#      cut, never blocks a gate run)
#
# Nothing is written to disk, and callers must never echo the result
# into command output: the sweep's stdout is a parsed contract (the
# health check renders verdicts from it and caches payloads), so a
# leaked token would poison the parse AND land in a cache file. A
# machine offering none of the three sources resolves empty and
# stays anonymous - honest, and the caller's skip message says so.
#
# Usage (sourced, never run):
#   source ".../lib/github_token.sh"
#   GITHUB_TOKEN="${GITHUB_TOKEN:-$(resolve_github_token)}"

# resolve_github_token - echo the first token this machine offers
# for the GitHub API, or nothing. Every probe is quiet, prompt-free,
# and time-boxed; a probe that fails for any reason falls through to
# the next source instead of failing the caller.
resolve_github_token() {
	if [ -n "${GITHUB_TOKEN:-}" ]; then
		printf '%s\n' "$GITHUB_TOKEN"
		return 0
	fi
	if command -v gh >/dev/null 2>&1; then
		local gh_token
		gh_token="$(timeout 5 gh auth token 2>/dev/null || true)"
		if [ -n "$gh_token" ]; then
			printf '%s\n' "$gh_token"
			return 0
		fi
	fi
	local credentials password
	credentials="$(printf 'protocol=https\nhost=github.com\n\n' |
		GIT_TERMINAL_PROMPT=0 timeout 5 git credential fill 2>/dev/null || true)"
	password="$(printf '%s\n' "$credentials" | sed -n 's/^password=//p' | head -n 1)"
	if [ -n "$password" ]; then
		printf '%s\n' "$password"
	fi
	return 0
}
