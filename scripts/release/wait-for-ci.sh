#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux). Runs on GitHub-hosted ubuntu runners
#   (curl + jq are runner staples); not for Windows cmd.exe.
#
# NIGHT-improve-1: the CI gate for tag-triggered workflows (release.yml,
# crates-io.yml). When the owner pushes the release commit and its tag
# together (git push origin main vX.Y.Z), the tag pipelines start while
# the branch CI (ci.yml) is still running on the same SHA — before this
# gate the release built and crates.io published with zero
# serialization, so a broken commit could ship before CI passed
# judgment (a crates.io publish is irreversible). This script
# serializes the two: it polls the GitHub Actions API until the ci.yml
# push-run for the exact tagged SHA completes, then requires
# conclusion=success before the caller proceeds.
#
# Semantics:
#   - A ci.yml run for the SHA is queued/in_progress -> keep polling.
#   - The newest completed run must have conclusion=success:
#       success            -> gate PASSES.
#       failure/timed_out/
#       startup_failure    -> classified FIRST (NIGHT-dinner-1, see
#                             below): GitHub-infra failures get up to
#                             WAIT_INFRA_RETRIES automatic re-runs of
#                             the failed jobs; everything else — or a
#                             budget already spent — blocks the release
#                             (the code the tag points at did not pass
#                             CI).
#       cancelled          -> gate FAILS with a recovery hint. A
#                             cancelled CI never verified the code (the
#                             usual cause: a newer main push cancelled
#                             it via ci.yml's cancel-in-progress). Re-run
#                             CI for the SHA, or re-push the tag.
#   - No ci.yml run exists after the grace period -> gate PASSES: the
#     commit touched nothing in ci.yml's `paths:` filter (a docs-only
#     release), so there is no run to wait for. The unconditional
#     cosmic-dragon-guard.yml still covered that push.
#
# NIGHT-dinner-1 — the infra-failure carve-out. GitHub's own
# infrastructure fails sometimes: a release-asset URL answering 500
# mid-download (observed in the owner's fleet: the shfmt pin step,
# curl exit 22, run dying before a single gate executed), a runner
# evicted mid-job, a workflow that never started. A red gate that is
# GitHub's fault must never read as a code fault — and an IRREVERSIBLE
# crates.io publish must not stall on it. When the waited run fails,
# this gate now fetches the failed jobs' logs and hunts for known
# infra signatures (curl 5xx/network classes, runner shutdown,
# download faults). ALL failed jobs signatured -> one re-run of the
# failed jobs is requested (needs permissions: actions: write) and
# polling resumes, up to WAIT_INFRA_RETRIES times. The safety
# property: classification can only BUY A RE-RUN — a real code failure
# fails the re-run too, burns the budget, and blocks exactly as
# before. Logs that cannot be fetched are treated as a real failure
# (never auto-retry what cannot be seen).
#
# Env (all provided by GitHub Actions):
#   GITHUB_API_URL, GITHUB_REPOSITORY, GITHUB_SHA, GITHUB_TOKEN
# Optional overrides:
#   WAIT_TIMEOUT_SECS  (default 1800) total polling budget
#   WAIT_GRACE_SECS    (default  90)  window for a sibling branch-push
#                                      event to register its CI run
#   WAIT_POLL_SECS     (default  30)  poll interval
#   WAIT_WORKFLOW_PATH (default .github/workflows/ci.yml)
#   WAIT_INFRA_RETRIES (default   2)  automatic re-run budget for
#                                      infra-signatured failures
#
# Usage (from a workflow job carrying permissions: actions: read for
# the wait itself; actions: write additionally unlocks the dinner-1
# automatic re-run of infra-signatured failures):
#   ./scripts/release/wait-for-ci.sh
set -euo pipefail

TIMEOUT_SECS="${WAIT_TIMEOUT_SECS:-1800}"
GRACE_SECS="${WAIT_GRACE_SECS:-90}"
POLL_SECS="${WAIT_POLL_SECS:-30}"
WORKFLOW_PATH="${WAIT_WORKFLOW_PATH:-.github/workflows/ci.yml}"

: "${GITHUB_API_URL:?GITHUB_API_URL is required}"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
: "${GITHUB_SHA:?GITHUB_SHA is required}"
: "${GITHUB_TOKEN:?GITHUB_TOKEN is required}"

deadline=$(($(date +%s) + TIMEOUT_SECS))
grace_until=$(($(date +%s) + GRACE_SECS))
infra_retries_left="${WAIT_INFRA_RETRIES:-2}"
# The re-run request budget's denominator for the attempt counter
# below (the default the left side started from).
WAIT_INFRA_RETRIES_DEFAULTED="${WAIT_INFRA_RETRIES:-2}"
# The run id the last re-run request targeted — re-POSTing for the
# same run while GitHub has not flipped it back to in_progress yet
# would burn the budget twice for one flake.
just_reran=""

# Newest ci.yml push-run for the tagged SHA, or "" when none exists.
# Output: "run_id|status|conclusion|html_url" (conclusion is "-" until
# the run completes; the run id is the dinner-1 re-run target). The
# run list is filtered to event=push + branch=main so
# pull_request-triggered runs of the same SHA never satisfy or fail
# the gate.
newest_run() {
	curl -fsSL \
		-H "Authorization: Bearer ${GITHUB_TOKEN}" \
		-H "Accept: application/vnd.github+json" \
		"${GITHUB_API_URL}/repos/${GITHUB_REPOSITORY}/actions/runs?event=push&branch=main&head_sha=${GITHUB_SHA}&per_page=50" |
		jq -r '[.workflow_runs[] | select(.path == "'"${WORKFLOW_PATH}"'")]
		        | sort_by(.run_number)
		        | .[-1]
		        | if . == null then "" else "\(.id)|\(.status)|\(.conclusion // "-")|\(.html_url)" end'
}

# Infra signatures (NIGHT-dinner-1): ERE alternation, matched
# case-insensitively against the failed jobs' logs. Every entry is a
# GitHub-side or network-side fault class, never a tree fault: the curl
# transport errors (7 connect, 18 partial file, 22 HTTP >= 400, 26 read
# timeout, 28 operation timeout, 35 TLS, 56 recv, 92 HTTP/2), the 5xx
# response texts, download faults (rustup's component downloads
# included), DNS and route failures, runner eviction, and the apt
# mirror's fetch failures. gate-keepers.sh itself exits only 0 or 1,
# so none of these can be a verdict the tree produced.
INFRA_SIGNATURES='curl: \(7\)|curl: \(18\)|curl: \(22\)|curl: \(26\)|curl: \(28\)|curl: \(35\)|curl: \(56\)|curl: \(92\)|the requested URL returned error: 5|500 Internal Server Error|502 Bad Gateway|503 Service Unavailable|504 Gateway Timeout|failed to download|could not download|download failed for|temporary failure resolving|could not resolve host|network is unreachable|connection reset by peer|connection timed out|operation timed out|the runner has received a shutdown signal|failed to fetch|unable to fetch some archives'

# Classify a completed, non-success run (NIGHT-dinner-1): does EVERY
# failed job carry an infra signature? Echoes the verdict to stdout;
# evidence and reasoning print to stderr so the caller's capture
# stays clean.
#       infra   — every failed job matched a signature, or the run failed
#               with zero failed jobs (startup_failure / runner-level
#               eviction: the failure happened outside any job's tree)
#       real    — at least one failed job shows no infra signature: the
#               tree itself failed; block exactly as before
#       opaque  — a failed job's logs could not be fetched; block (the
#               safe default: never auto-retry what cannot be seen)
classify_failed_run() {
	run_id="$1"
	jobs="$(curl -fsSL \
		-H "Authorization: Bearer ${GITHUB_TOKEN}" \
		-H "Accept: application/vnd.github+json" \
		"${GITHUB_API_URL}/repos/${GITHUB_REPOSITORY}/actions/runs/${run_id}/jobs?per_page=100" |
		jq -r '[.jobs[] | select(.conclusion == "failure" or .conclusion == "timed_out") | .id] | join(" ")' 2>/dev/null)" || jobs=""
	if [[ -z "${jobs}" ]]; then
		echo "[ci-gate] run ${run_id}: failed with no failed jobs — a startup/runner-level fault (GitHub's side)." >&2
		echo "infra"
		return
	fi
	saw_opaque=0
	for job_id in ${jobs}; do
		log="$(curl -fsSL \
			-H "Authorization: Bearer ${GITHUB_TOKEN}" \
			-H "Accept: application/vnd.github+json" \
			"${GITHUB_API_URL}/repos/${GITHUB_REPOSITORY}/actions/jobs/${job_id}/logs" 2>/dev/null)" || log=""
		if [[ -z "${log}" ]]; then
			saw_opaque=1
			echo "[ci-gate] failed job ${job_id}: logs unavailable (expired?) — not classifying it as infra." >&2
			continue
		fi
		hits="$(grep -E -i -m 3 "${INFRA_SIGNATURES}" <<<"${log}" || true)"
		if [[ -z "${hits}" ]]; then
			echo "[ci-gate] failed job ${job_id}: no infra signature — the tree itself failed." >&2
			echo "real"
			return
		fi
		echo "[ci-gate] failed job ${job_id}: infra signature evidence:" >&2
		while IFS= read -r hit_line; do
			echo "[ci-gate]   ${hit_line}" >&2
		done <<<"${hits}"
	done
	if [[ "${saw_opaque}" -eq 1 ]]; then
		echo "opaque"
		return
	fi
	echo "infra"
}

# Re-trigger a failed run (NIGHT-dinner-1): the failed jobs only
# when GitHub exposes a failed-jobs set for it, otherwise the whole
# run (a startup_failure has no failed jobs to target). Requires the
# calling job to carry permissions: actions: write.
retrigger_run() {
	run_id="$1"
	if curl -fsSL -X POST \
		-H "Authorization: Bearer ${GITHUB_TOKEN}" \
		-H "Accept: application/vnd.github+json" \
		"${GITHUB_API_URL}/repos/${GITHUB_REPOSITORY}/actions/runs/${run_id}/rerun-failed-jobs" >/dev/null 2>&1; then
		echo "[ci-gate] re-run requested: failed jobs of run ${run_id}."
		return 0
	fi
	if curl -fsSL -X POST \
		-H "Authorization: Bearer ${GITHUB_TOKEN}" \
		-H "Accept: application/vnd.github+json" \
		"${GITHUB_API_URL}/repos/${GITHUB_REPOSITORY}/actions/runs/${run_id}/rerun" >/dev/null 2>&1; then
		echo "[ci-gate] no failed-jobs set to target — re-ran the WHOLE run ${run_id}."
		return 0
	fi
	return 1
}

echo "[ci-gate] workflow: ${WORKFLOW_PATH}"
echo "[ci-gate] sha:      ${GITHUB_SHA}"
echo "[ci-gate] budget:   ${TIMEOUT_SECS}s (grace ${GRACE_SECS}s, poll ${POLL_SECS}s)"

while :; do
	summary="$(newest_run)"

	# NIGHT-diner-10 regression fix (zelynic NIGHT-dinner-25
	# lineage — the same bug, found there first on the v11.0.0-rc.3
	# tag): the gate shipped in 041b470 with the run id already
	# prepended to newest_run's output (dinner-1's re-run
	# targeting) while the branch test below still matched the
	# pre-prepend prefix `completed|*`. The summary starts with
	# the numeric run id, so that branch was dead code from
	# birth: every completed run — green or red — fell into the
	# waiting branch and polled straight into the budget
	# timeout while the log printed "conclusion so far: success"
	# over and over (exactly what v100.0.6-rc.1's two tag
	# pipelines did while CI for the tagged SHA sat green). The
	# fields are parsed once here; the status FIELD is the
	# branch, never the prefix.
	IFS='|' read -r run_id status conclusion url <<<"${summary}"

	if [[ -z "${summary}" ]]; then
		if (($(date +%s) >= grace_until)); then
			echo "[ci-gate] PASS: no ci.yml push-run exists for this SHA after the grace period."
			echo "[ci-gate] (ci.yml's paths filter skipped this commit — a docs-only release; the"
			echo "[ci-gate]  unconditional cosmic-dragon-guard.yml workflow still covered the push.)"
			exit 0
		fi
		echo "[ci-gate] no ci.yml run visible yet (commit+tag pushed together?); polling..."
	elif [[ "${status}" == "completed" ]]; then
		if [[ "${conclusion}" == "success" ]]; then
			echo "[ci-gate] PASS: ci.yml completed with conclusion=success."
			echo "[ci-gate] run: ${url}"
			exit 0
		fi
		# NIGHT-dinner-1: before blocking an IRREVERSIBLE release or
		# publish on a red run, ask WHY it is red (see the header).
		# Infra verdicts buy automatic re-runs of the failed jobs;
		# real / opaque verdicts — and a spent budget — block exactly
		# as before.
		verdict="$(classify_failed_run "${run_id}")"
		if [[ "${verdict}" == "infra" ]]; then
			if [[ "${just_reran}" == "${run_id}" ]]; then
				echo "[ci-gate] re-run of run ${run_id} requested; GitHub has not flipped it back yet — polling..."
			elif [[ "${infra_retries_left}" -gt 0 ]]; then
				infra_retries_left=$((infra_retries_left - 1))
				attempt=$((WAIT_INFRA_RETRIES_DEFAULTED - infra_retries_left))
				echo "::warning::ci.yml concluded '${conclusion}' with GitHub-infra signatures — automatic re-run ${attempt}/${WAIT_INFRA_RETRIES_DEFAULTED}."
				if ! retrigger_run "${run_id}"; then
					echo "::error::the re-run request itself failed (does this job carry permissions: actions: write?)."
					echo "::error::Blocking as a hard failure — the tree may be fine; re-run this gate job after fixing the token."
					echo "::error::run: ${url}"
					exit 1
				fi
				just_reran="${run_id}"
				sleep "${POLL_SECS}"
				continue
			else
				echo "::error::ci.yml kept failing with GitHub-infra signatures after ${WAIT_INFRA_RETRIES_DEFAULTED} automatic re-run(s) —"
				echo "::error::GitHub's side is failing right now, not the code. Re-run this gate job later, or re-push the tag once it recovers."
				echo "::error::run: ${url}"
				exit 1
			fi
		fi
		echo "::error::ci.yml concluded '${conclusion}' for the tagged SHA — the release pipeline is blocked."
		echo "::error::run: ${url}"
		if [[ "${conclusion}" == "cancelled" ]]; then
			echo "::error::A cancelled CI never verified this code (a newer main push likely"
			echo "::error::cancelled it via ci.yml's cancel-in-progress). Re-run CI for the SHA"
			echo "::error::(Actions UI or: gh run rerun <run-id>), then re-run this gate job —"
			echo "::error::or re-push the tag."
		fi
		exit 1
	else
		echo "[ci-gate] ci.yml run is ${status} (conclusion so far: ${conclusion}); waiting ${POLL_SECS}s..."
	fi

	if (($(date +%s) + POLL_SECS > deadline)); then
		echo "::error::ci-gate timed out after ${TIMEOUT_SECS}s waiting for ci.yml on ${GITHUB_SHA}."
		echo "::error::Re-run this gate job once CI finishes, or push the tag again."
		exit 1
	fi
	sleep "${POLL_SECS}"
done
