# shellcheck shell=bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Sourced by
#   scripts/build/build.sh — not a standalone executable (no main, no
#   argument parsing; the entry script owns both).
#
# COSMOSTRIX BUILD MODULE: miri
# Miri (UB detector) integration: stamp file contract, the status
# banner shown on every build, and the nightly-driven test runner with
# its 6 audited pure-Rust module scope.
#
# Part of the build.sh split (NIGHT-lts-2): the single-entry orchestrator
# was de-monolithed into sourced modules; function bodies are byte-identical
# moves from the former flat script.

# ── Miri (UB detector) integration ────────────────────────────────────
# Miri runs under nightly and verifies unsafe code is sound. The full
# test suite under Miri takes 30+ min and many tests touch TTY/FFI which
# Miri cannot run. We restrict to the 6 pure-Rust modules already audited
# in docs/archive/audits/UNSAFE_SOUNDNESS_AUDIT.md:
#   - config_hints::tests  (41 tests)
#   - validation::tests    (18 tests)
#   - color_cache::tests   (12 tests)
#   - safepath::tests      (22 tests)
#   - humanize::tests      (9 tests)
#   - bolt::tests          (5 tests)
#
# Verification status is cached in target/miri-stamp (key=value format)
# so we don't re-run on every build. A status banner is shown at the
# start of every build.sh invocation reflecting the stamp state:
#   - VERIFIED  — stamp commit == HEAD
#   - STALE     — stamp commit != HEAD (suggests re-run)
#   - FAILED    — last run had violations
#   - NEVER RUN — no stamp yet

readonly MIRI_STAMP_FILE="target/miri-stamp"
readonly MIRI_LOG_FILE="target/miri-log.txt"
readonly MIRI_AUDIT_MODULES=(
	"config_hints::"
	"validation::"
	"color_cache::"
	"safepath::"
	"humanize::"
	"bolt::"
)

# Print one-line Miri status banner. Called from main() before dispatch.
# Quiet when --quiet-miri is passed (CI jobs that don't care), and also
# quiet under --quiet/-q (NIGHT-enhanced-3) since the banner is
# informational status output, not a failure/warning.
show_miri_status() {
	[ "${MIRI_QUIET:-0}" = "1" ] && return 0
	[ "${QUIET_CHECK:-0}" = "1" ] && return 0

	local head head_short
	head=$(git rev-parse HEAD 2>/dev/null || echo "")
	head_short=$(git rev-parse --short HEAD 2>/dev/null || echo "")

	if [ ! -f "${MIRI_STAMP_FILE}" ]; then
		log_info "Miri: never run on this workspace. Run './scripts/build/build.sh miri' to verify (needs nightly, ~3-10 min)."
		return 0
	fi

	# Parse stamp (key=value format, no jq dependency)
	local stamp_commit stamp_short stamp_ts_iso stamp_status stamp_dur stamp_run stamp_fail
	stamp_commit=$(grep '^commit=' "${MIRI_STAMP_FILE}" | cut -d= -f2-)
	stamp_short=$(grep '^commit_short=' "${MIRI_STAMP_FILE}" | cut -d= -f2-)
	stamp_ts_iso=$(grep '^timestamp_iso=' "${MIRI_STAMP_FILE}" | cut -d= -f2-)
	stamp_status=$(grep '^status=' "${MIRI_STAMP_FILE}" | cut -d= -f2-)
	stamp_dur=$(grep '^duration_ms=' "${MIRI_STAMP_FILE}" | cut -d= -f2-)
	stamp_run=$(grep '^tests_run=' "${MIRI_STAMP_FILE}" | cut -d= -f2-)
	stamp_fail=$(grep '^tests_failed=' "${MIRI_STAMP_FILE}" | cut -d= -f2-)

	local dur_sec=""
	if [ -n "${stamp_dur}" ]; then
		dur_sec=$(awk -v ms="${stamp_dur}" 'BEGIN { printf "%.1f", ms/1000 }')
	fi

	case "${stamp_status}" in
	verified)
		if [ "${stamp_commit}" = "${head}" ]; then
			log_success "Miri VERIFIED at ${stamp_short} · ${stamp_ts_iso} · ${stamp_run} tests (${stamp_fail} fail) · ${dur_sec}s"
		else
			log_warning "Miri STALE — verified at ${stamp_short}, HEAD is ${head_short}. Run './scripts/build/build.sh miri' to refresh."
		fi
		;;
	failed)
		log_error "Miri FAILED at ${stamp_short} · ${stamp_ts_iso} · ${stamp_fail} violations. See ${MIRI_LOG_FILE}."
		;;
	skipped:*)
		local reason="${stamp_status#skipped:}"
		log_info "Miri SKIPPED (${reason}) at ${stamp_short}. Run './scripts/build/build.sh miri' to verify."
		;;
	*)
		log_info "Miri: unknown status '${stamp_status}' at ${stamp_short}."
		;;
	esac
}

# Run Miri verification on the pure-Rust modules. Updates stamp file.
# Args (parsed globally before main):
#   --filter <pat>  Narrow test scope (substring match, e.g. "validation::")
#   --no-install    Don't auto-install nightly/miri (fail if missing)
#   --full          Run entire lib test suite (slow, may fail on FFI tests)
run_miri() {
	local filter="${MIRI_FILTER:-}"
	local full="${MIRI_FULL:-0}"

	# 1. rustup is required (needed for nightly toolchain).
	if ! command -v rustup &>/dev/null; then
		log_error "rustup not installed. Miri requires nightly. Install from https://rustup.rs"
		exit 1
	fi

	# 2. Ensure nightly toolchain is installed.
	if ! rustup toolchain list 2>/dev/null | grep -q '^nightly-'; then
		if [ "${MIRI_NO_INSTALL}" = "1" ]; then
			log_error "nightly toolchain not installed and --no-install given. Aborting."
			exit 1
		fi
		log_step "Installing nightly toolchain (rustup toolchain install nightly --profile minimal --component miri)..."
		rustup toolchain install nightly --profile minimal --component miri || {
			log_error "Failed to install nightly + miri. Try manually: rustup toolchain install nightly --component miri"
			exit 1
		}
	else
		# 3. Ensure miri component is installed on nightly.
		# Modern rustup emits per-target lines like
		# `miri-x86_64-unknown-linux-gnu (installed)`, so the
		# separator after `miri` can be either a hyphen (newer
		# rustup) or a space (older rustup). Match both.
		if ! rustup component list --toolchain nightly 2>/dev/null | grep -q '^miri[-[:space:]].*installed'; then
			if [ "${MIRI_NO_INSTALL}" = "1" ]; then
				log_error "miri component not installed on nightly and --no-install given. Aborting."
				exit 1
			fi
			log_step "Installing miri component on nightly..."
			rustup component add miri --toolchain nightly || {
				log_error "Failed to install miri component."
				exit 1
			}
		fi
	fi

	local nightly_ver miri_ver
	nightly_ver=$(rustc +nightly --version 2>/dev/null | head -1)
	miri_ver=$(cargo +nightly miri --version 2>/dev/null | head -1)
	log_info "Miri: ${miri_ver}"
	log_info "Nightly: ${nightly_ver}"

	# 4. One-time libstd setup (non-interactive) to avoid prompt during test run.
	log_step "Ensuring Miri sysroot is set up (cargo +nightly miri setup)..."
	cargo +nightly miri setup 2>&1 | tail -3 || true

	# 5. Build the test filter list.
	local filter_args=()
	if [ -n "${filter}" ]; then
		filter_args=("${filter}")
		log_step "Running Miri with filter: ${filter}"
	elif [ "${full}" = "1" ]; then
		# No filter — run entire lib test suite.
		log_step "Running Miri on FULL lib test suite (this is slow, may fail on FFI tests)..."
	else
		# Default: 6 known-good pure-Rust modules.
		read -r -a filter_args <<<"${MIRI_AUDIT_MODULES[*]}"
		log_step "Running Miri on ${#filter_args[@]} audited pure-Rust modules: ${filter_args[*]}"
	fi

	# 6. Run Miri. MIRIFLAGS disables isolation so tests that need
	#    env vars / time / file paths don't fail spuriously.
	#    Note: cosmostrix is a binary crate (no src/lib.rs), so we
	#    don't pass --lib. The default test target covers all
	#    unittests: declared from src/ modules, with many module
	#    bodies living in the mirrored test/ tree via #[path]
	#    includes (NIGHT-hunter-1).
	local start_ms end_ms duration_ms
	start_ms=$(date +%s%3N 2>/dev/null || date +%s)

	mkdir -p target
	local miri_exit=0
	MIRIFLAGS="${MIRIFLAGS:--Zmiri-disable-isolation}" \
		cargo +nightly miri test -- "${filter_args[@]}" \
		2>&1 | tee "${MIRI_LOG_FILE}" || miri_exit=$?

	end_ms=$(date +%s%3N 2>/dev/null || date +%s)
	duration_ms=$((end_ms - start_ms))

	# 7. Parse test result counts from log (sum across all test binaries).
	local tests_run=0 tests_failed=0
	tests_run=$(grep -oE '[0-9]+ passed' "${MIRI_LOG_FILE}" 2>/dev/null | awk '{s+=$1} END {print s+0}')
	tests_failed=$(grep -oE '[0-9]+ failed' "${MIRI_LOG_FILE}" 2>/dev/null | awk '{s+=$1} END {print s+0}')

	# 8. Determine final status.
	local status
	if [ "${miri_exit}" = "0" ]; then
		status="verified"
		log_success "Miri verification PASSED (${tests_run} tests, 0 fail, ${duration_ms}ms)"
	else
		status="failed"
		log_error "Miri verification FAILED (${tests_failed} failures, ${duration_ms}ms). See ${MIRI_LOG_FILE}."
	fi

	# 9. Write stamp file. Skip stamp update if --filter or --full was used
	#    (partial runs don't represent the audited scope).
	if [ -n "${filter}" ] || [ "${full}" = "1" ]; then
		log_info "Partial run — stamp file not updated. Run './scripts/build/build.sh miri' (no flags) to refresh."
	else
		local head head_short ts_iso ts_unix
		head=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
		head_short=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
		ts_unix=$(date +%s)
		ts_iso=$(date -u +%Y-%m-%dT%H:%M:%SZ)

		cat >"${MIRI_STAMP_FILE}" <<STAMP_EOF
# cosmostrix Miri verification stamp (generated by scripts/build/build.sh miri)
# Format: key=value. Parse with grep '^key=' | cut -d= -f2-
commit=${head}
commit_short=${head_short}
timestamp_unix=${ts_unix}
timestamp_iso=${ts_iso}
status=${status}
duration_ms=${duration_ms}
tests_run=${tests_run}
tests_failed=${tests_failed}
modules=$(
			IFS=','
			echo "${MIRI_AUDIT_MODULES[*]}"
		)
miri_version=${miri_ver}
nightly_version=${nightly_ver}
STAMP_EOF
		log_info "Stamp written to ${MIRI_STAMP_FILE}"
		log_info "Log saved to ${MIRI_LOG_FILE}"
	fi

	if [ "${miri_exit}" != "0" ]; then
		exit 1
	fi
}
