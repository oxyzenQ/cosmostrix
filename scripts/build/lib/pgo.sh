# shellcheck shell=bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Sourced by
#   scripts/build/build.sh — not a standalone executable (no main, no
#   argument parsing; the entry script owns both).
#
# COSMOSTRIX BUILD MODULE: pgo
# PGO (Profile-Guided Optimization) nitro build: CPU detection,
# the 3-stage pipeline (instrument, train, optimize), stage stamps for
# crash recovery, log rotation, profile merging, summary and delta
# validation.
#
# Part of the build.sh split (NIGHT-lts-2): the single-entry orchestrator
# was de-monolithed into sourced modules; function bodies are byte-identical
# moves from the former flat script.

# ── PGO (Profile-Guided Optimization) nitro build ───────────────────────
# Two-stage: instrument → benchmark → recompile with profile data.
# Expected gain: 5-15% FPS improvement over the pro profile.

# Detect the best -C target-cpu value for the host machine.
#   - Linux x86_64: reads /proc/cpuinfo for avx512f / avx2 flags
#   - macOS x86_64: uses sysctl machdep.cpu.features
#   - aarch64 / other: returns "native" (LLVM already tunes well for ARM)
# Echoes the target-cpu value (suitable for COSMOSTRIX_TARGET_CPU).
detect_cpu_target() {
	local arch
	arch="$(uname -m 2>/dev/null || echo unknown)"

	# ARM and other non-x86 architectures: LLVM's codegen already
	# produces excellent ARM64 code without PGO-specific tuning, and
	# there is no equivalent of the x86-64-vN microarchitecture
	# levels. Stick with native.
	if [ "${arch}" != "x86_64" ] && [ "${arch}" != "amd64" ]; then
		echo "native"
		return 0
	fi

	local flags=""
	if [ -r /proc/cpuinfo ]; then
		# Linux: grep the flags line from the first CPU entry.
		# The line is space-separated, so grep -o is safe.
		flags="$(grep -m1 -E '^flags' /proc/cpuinfo 2>/dev/null | cut -d: -f2- || true)"
	elif command -v sysctl >/dev/null 2>&1; then
		# macOS: machdep.cpu.features is a space-separated list,
		# uppercased (e.g. AVX2, AVX512F).
		flags="$(sysctl -n machdep.cpu.features 2>/dev/null || true)"
	fi

	# Normalize to lowercase for case-insensitive matching.
	flags="$(echo "${flags}" | tr '[:upper:]' '[:lower:]')"

	if echo "${flags}" | grep -qw avx512f; then
		echo "x86-64-v4"
	elif echo "${flags}" | grep -qw avx2; then
		echo "x86-64-v3"
	else
		# No AVX2 — fall back to native rather than guessing v1/v2.
		# native lets rustc pick the host's actual feature set.
		echo "native"
	fi
}

build_pgo() {
	log_step "Starting PGO nitro build (3-stage: instrument → profile → optimize)"

	local pgo_dir="${PWD}/target/pgo-data"
	local pgo_stages_dir="${PWD}/target/pgo-stages"
	local pgo_logs_dir="${PWD}/target/pgo-logs"
	local instrument_bin="target/${TARGET}/pgo-instrument/${PROJECT_NAME}"
	local nitro_bin="target/${TARGET}/pgo-use/${PROJECT_NAME}"
	local pgo_log_file
	pgo_log_file="${pgo_logs_dir}/pgo-$(date +%Y%m%d-%H%M%S).log"

	# ── Pre-flight checks ──────────────────────────────────────────
	# Fail fast on missing tools or insufficient disk space before
	# we spend 5+ minutes building the instrumented binary.
	pgo_preflight_check "${pgo_dir}"

	# Set up log directory + rotate old logs (keep last 5)
	mkdir -p "${pgo_logs_dir}"
	pgo_rotate_logs "${pgo_logs_dir}"

	# Tee all subsequent output to the log file (and stdout).
	# Use exec to redirect the script's stdout/stderr, but keep stdout
	# visible via tee.
	exec > >(tee -a "${pgo_log_file}") 2>&1
	log_info "PGO training log: ${pgo_log_file}"

	# Capture environment for reproducibility (written to log via tee)
	log_info "── PGO Environment ──"
	log_info "  rustc: $(rustc --version 2>/dev/null || echo 'unknown')"
	log_info "  cargo: $(cargo --version 2>/dev/null || echo 'unknown')"
	log_info "  host:  $(uname -s -m -r 2>/dev/null || echo 'unknown')"
	log_info "  target: ${TARGET}"
	log_info "  jobs: ${MAX_JOBS}"
	log_info "  commit: $(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')"
	log_info "  validate: $([ "${PGO_VALIDATE}" -eq 1 ] && echo 'yes (will build release baseline)' || echo 'no')"
	log_info "  resume: $([ "${PGO_NO_RESUME}" -eq 1 ] && echo 'disabled (--no-resume)' || echo 'enabled')"

	# PGO target CPU resolution — TWO separate targets:
	#
	#   instrument_cpu  → used for the Stage 1 instrumented binary, which
	#                     MUST EXECUTE on the build host to collect profile
	#                     data. Must be a target the host can actually run.
	#   final_cpu       → used for the Stage 3 optimized binary, which is
	#                     shipped to users. Can be more aggressive than
	#                     instrument_cpu because it never runs on the host.
	#
	# Why split them? Profile data captures branch frequencies and hot
	# paths — it is independent of SIMD codegen. So a v3-built instrumented
	# binary produces valid profile data for a v4-built final binary.
	# This lets CI build v4 PGO binaries even when the runner lacks
	# AVX-512 (which would otherwise SIGILL the instrumented binary and
	# abort Stage 2 with no profile data collected).
	#
	# final_cpu priority: COSMOSTRIX_TARGET_CPU env > --auto detect > native.
	# The --auto flag (set by `cargo use-pgo`) probes the host CPU and
	# selects the highest x86-64 microarchitecture level it supports.
	if [ -z "${COSMOSTRIX_TARGET_CPU:-}" ] && [ "${PGO_AUTO}" -eq 1 ]; then
		local detected
		detected="$(detect_cpu_target)"
		export COSMOSTRIX_TARGET_CPU="${detected}"
		log_info "Auto-detected final CPU target: ${detected}"
	fi
	local final_cpu="${COSMOSTRIX_TARGET_CPU:-native}"

	# instrument_cpu priority: COSMOSTRIX_INSTRUMENT_CPU env > safe default.
	# Safe default: x86-64-v3 on x86_64 (universally supported on every
	# modern x86_64 host including GitHub Actions runners, which lack
	# AVX-512), native elsewhere (LLVM handles ARM tuning well).
	local instrument_cpu="${COSMOSTRIX_INSTRUMENT_CPU:-}"
	if [ -z "${instrument_cpu}" ]; then
		local instr_arch
		instr_arch="$(uname -m 2>/dev/null || echo unknown)"
		if [ "${instr_arch}" = "x86_64" ] || [ "${instr_arch}" = "amd64" ]; then
			instrument_cpu="x86-64-v3"
		else
			instrument_cpu="native"
		fi
	fi
	log_info "Instrument CPU: ${instrument_cpu} (must run on build host)"
	log_info "Final CPU: ${final_cpu}"

	mkdir -p "${pgo_dir}" "${pgo_stages_dir}"

	# ── Stage 1: Build instrumented binary ─────────────────────────
	# Crash recovery: if stage-1 stamp exists + commit matches + binary
	# exists, skip Stage 1 (saves ~60s on re-runs after Stage 2/3 failure).
	local stage1_stamp="${pgo_stages_dir}/stage-1-instrument"
	if [ "${PGO_NO_RESUME}" -eq 0 ] && pgo_stage_stamp_valid "${stage1_stamp}" "${instrument_cpu}" "${final_cpu}"; then
		if [ -f "${instrument_bin}" ]; then
			log_success "Stage 1/3: SKIPPED (stamp valid + binary exists)"
			log_info "  Use --no-resume to force rebuild"
		else
			log_warning "Stage 1 stamp valid but binary missing — rebuilding"
			pgo_stage_run_1 "${pgo_dir}" "${instrument_cpu}" "${instrument_bin}"
		fi
	else
		pgo_stage_run_1 "${pgo_dir}" "${instrument_cpu}" "${instrument_bin}"
	fi
	pgo_stage_stamp_write "${stage1_stamp}" "${instrument_cpu}" "${final_cpu}"

	# ── Stage 2: Run multi-workload PGO training ───────────────────
	# Multi-workload PGO training:
	#   1. monolith + zen (default, peak throughput — train hot loop)
	#   2. cinematic + katana (heavier scene, glyph pool — train glyph-emit)
	#   3. signal + binary (glyph scene with anomaly zones — train post-fx)
	#   4. screensaver + message box (train message-box overlay + screensaver)
	#
	# All runs use --bench-io (wet I/O) so the BenchIoWriter +
	# VisualSampler + clear_dirty paths are exercised. This was the
	# biggest gap in the old training — the wet path was completely
	# untrained, causing PGO to emit suboptimal code for it.
	#
	# Crash recovery: if stage-2 stamp exists + commit matches + at least
	# 4 profraw files exist, skip Stage 2 (saves ~50s on re-runs after
	# Stage 3 failure).
	local stage2_stamp="${pgo_stages_dir}/stage-2-train"
	local stage2_skip=0
	if [ "${PGO_NO_RESUME}" -eq 0 ] && pgo_stage_stamp_valid "${stage2_stamp}" "${instrument_cpu}" "${final_cpu}"; then
		local profraw_count
		profraw_count=$(find "${pgo_dir}" -name "*.profraw" 2>/dev/null | wc -l)
		if [ "${profraw_count}" -ge 4 ]; then
			log_success "Stage 2/3: SKIPPED (stamp valid + ${profraw_count} profraw files exist)"
			log_info "  Use --no-resume to force retraining"
			stage2_skip=1
		fi
	fi

	if [ "${stage2_skip}" -eq 0 ]; then
		log_info "Stage 2/3: Running multi-workload PGO training (~50s total)..."
		if [ ! -f "${instrument_bin}" ]; then
			log_error "Stage 2 failed: instrumented binary not found at ${instrument_bin}"
			exit 1
		fi

		# Export LLVM_PROFILE_FILE so each invocation writes to a unique
		# .profraw file. %p = PID, %m = module hash. Without this, concurrent
		# or sequential runs would overwrite each other's profile data.
		export LLVM_PROFILE_FILE="${pgo_dir}/cosmostrix-%p-%m.profraw"

		local train_failed=0

		# Workload 1: monolith + zen (default, peak throughput)
		pgo_run_workload 1 4 "monolith + zen" \
			"${instrument_bin}" --benchmark -C zen --bench-io --bench-duration 20 --scene monolith ||
			train_failed=1

		# Workload 2: cinematic + katana (heavier scene, larger glyph pool)
		pgo_run_workload 2 4 "cinematic + katakana" \
			"${instrument_bin}" --benchmark -C katakana --bench-io --bench-duration 12 --scene cinematic ||
			train_failed=1

		# Workload 3: signal + binary (anomaly zones, post-fx path)
		pgo_run_workload 3 4 "signal + binary" \
			"${instrument_bin}" --benchmark -C binary --bench-io --bench-duration 10 --scene signal ||
			train_failed=1

		# Workload 4: screensaver + message box (overlay + screensaver path)
		pgo_run_workload 4 4 "screensaver + message box" \
			"${instrument_bin}" --benchmark --bench-io --bench-duration 8 --screensaver -mb "pgo training" ||
			train_failed=1

		unset LLVM_PROFILE_FILE

		local profile_count
		profile_count=$(find "${pgo_dir}" -name "*.profraw" 2>/dev/null | wc -l)
		if [ "${profile_count}" -eq 0 ]; then
			log_error "Stage 2 failed: no profile data collected in ${pgo_dir}"
			log_info "Hint: ensure the benchmark ran for at least 5 seconds"
			log_info "Hint: check LLVM_PROFILE_FILE was exported correctly"
			log_info "Hint: see ${pgo_log_file} for per-workload output"
			exit 1
		fi
		log_success "Stage 2 complete: ${profile_count} profile file(s) collected from 4 workloads"
		if [ "${train_failed}" -ne 0 ]; then
			log_warning "Some workloads exited non-zero — profile coverage may be partial"
		fi
		pgo_stage_stamp_write "${stage2_stamp}" "${instrument_cpu}" "${final_cpu}"
	fi

	# ── Merge profile data (robust — skips corrupt profraw files) ──
	local profdata_file="${pgo_dir}/cosmostrix.profdata"
	pgo_merge_profiles "${pgo_dir}" "${profdata_file}"

	# ── Stage 3: Build optimized binary with profile data ──────────
	local stage3_stamp="${pgo_stages_dir}/stage-3-optimize"
	if [ "${PGO_NO_RESUME}" -eq 0 ] && pgo_stage_stamp_valid "${stage3_stamp}" "${instrument_cpu}" "${final_cpu}"; then
		if [ -f "${nitro_bin}" ]; then
			log_success "Stage 3/3: SKIPPED (stamp valid + binary exists)"
			log_info "  Use --no-resume to force rebuild"
		else
			log_warning "Stage 3 stamp valid but binary missing — rebuilding"
			pgo_stage_run_3 "${final_cpu}" "${profdata_file}" "${nitro_bin}"
		fi
	else
		pgo_stage_run_3 "${final_cpu}" "${profdata_file}" "${nitro_bin}"
	fi
	pgo_stage_stamp_write "${stage3_stamp}" "${instrument_cpu}" "${final_cpu}"

	local size
	size=$(du -h "${nitro_bin}" | cut -f1)
	log_success "PGO nitro build complete (${size})"

	# ── Summary report ─────────────────────────────────────────────
	pgo_print_summary "${nitro_bin}" "${instrument_bin}" "${pgo_dir}" "${profdata_file}" \
		"${instrument_cpu}" "${final_cpu}" "${pgo_log_file}"

	# ── Optional: delta validation (PGO vs release) ────────────────
	if [ "${PGO_VALIDATE}" -eq 1 ]; then
		pgo_validate_delta "${nitro_bin}" "${final_cpu}"
	else
		echo ""
		log_info "PGO gain: expected 5-15% FPS improvement over pro profile"
		log_info "Run: ${nitro_bin} --benchmark to measure"
		log_info "Add --validate to automatically build release baseline + compare FPS"
	fi
}

# ── PGO helper functions ────────────────────────────────────────────────

# Pre-flight checks: verify required tools + disk space before starting.
# Fails fast on missing llvm-profdata or insufficient disk space.
pgo_preflight_check() {
	local pgo_dir="$1"

	log_step "Pre-flight checks..."

	# 1. rustc supports -C profile-generate (stable since Rust 1.0,
	#    but verify the flag is recognized)
	# NOTE: capture output to a variable first, then grep. Direct pipe
	# (`rustc -C help | grep -q`) fails under `set -o pipefail` because
	# grep -q exits early, rustc gets SIGPIPE, and pipefail makes the
	# whole pipe fail.
	local rustc_help
	rustc_help=$(rustc -C help 2>/dev/null || true)
	if ! echo "${rustc_help}" | grep -q 'profile-generate'; then
		log_error "rustc does not support -C profile-generate. PGO requires Rust stable."
		exit 1
	fi
	log_success "  rustc supports -C profile-generate"

	# 2. llvm-profdata available (either in PATH or via rustup llvm-tools)
	local profdata_tool=""
	if command -v llvm-profdata >/dev/null 2>&1; then
		profdata_tool="llvm-profdata"
	else
		local rustup_profdata
		rustup_profdata="$(rustc --print sysroot 2>/dev/null)/lib/rustlib/$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')/bin/llvm-profdata"
		if [ -x "${rustup_profdata}" ]; then
			profdata_tool="${rustup_profdata}"
		fi
	fi
	if [ -z "${profdata_tool}" ]; then
		log_error "llvm-profdata not found. Install with: rustup component add llvm-tools-preview"
		log_error "If already installed, the rust-toolchain.toml override may be hiding it."
		log_error "The binary lives at: \$(rustc --print sysroot)/lib/rustlib/\$(rustc -vV | sed -n 's/^host: //p')/bin/llvm-profdata"
		exit 1
	fi
	log_success "  llvm-profdata: ${profdata_tool}"

	# 3. Disk space check — PGO data can be 100s of MB
	local avail_mb
	avail_mb=$(df -m "${pgo_dir%/*}" 2>/dev/null | awk 'NR==2 {print $4}' || echo 0)
	if [ "${avail_mb}" -gt 0 ] && [ "${avail_mb}" -lt 500 ]; then
		log_error "Insufficient disk space: ${avail_mb}MB available, need at least 500MB for PGO data"
		log_info "PGO data lives in ${pgo_dir} (~100-300MB typical)"
		exit 1
	fi
	if [ "${avail_mb}" -gt 0 ]; then
		log_success "  Disk space: ${avail_mb}MB available"
	else
		log_warning "  Could not determine disk space (df failed) — proceeding anyway"
	fi

	# 4. Target directory writable
	if ! mkdir -p "${pgo_dir}" 2>/dev/null; then
		log_error "Cannot create PGO data directory: ${pgo_dir}"
		exit 1
	fi
	log_success "  PGO data directory writable: ${pgo_dir}"
}

# Run a single PGO training workload with logging + crash tolerance.
# Args: workload_index total_workloads description bin [args...]
# Captures stdout/stderr to target/pgo-logs/workload-N.log
# Returns 0 on success, 1 on non-zero exit (does NOT abort the pipeline).
pgo_run_workload() {
	local idx="$1"
	local total="$2"
	local desc="$3"
	shift 3
	local bin="$1"
	shift

	local workload_log="${PWD}/target/pgo-logs/workload-${idx}.log"
	log_info "  [${idx}/${total}] ${desc}"

	if "$bin" "$@" >"${workload_log}" 2>&1; then
		log_success "    workload ${idx} completed"
		return 0
	else
		local exit_code=$?
		log_warning "    workload ${idx} exited non-zero (exit ${exit_code}) — see ${workload_log}"
		# Don't abort — partial profile data is still useful
		return 1
	fi
}

# Robust profile merge: skip corrupt/empty profraw files, report stats.
# Args: pgo_dir profdata_file
pgo_merge_profiles() {
	local pgo_dir="$1"
	local profdata_file="$2"

	local profdata_tool=""
	if command -v llvm-profdata >/dev/null 2>&1; then
		profdata_tool="llvm-profdata"
	else
		local rustup_profdata
		rustup_profdata="$(rustc --print sysroot 2>/dev/null)/lib/rustlib/$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')/bin/llvm-profdata"
		if [ -x "${rustup_profdata}" ]; then
			profdata_tool="${rustup_profdata}"
		fi
	fi

	if [ -z "${profdata_tool}" ]; then
		log_warning "llvm-profdata not found. Using raw profdata directory (rustc can handle this)"
		return
	fi

	log_info "Merging profile data with ${profdata_tool}..."

	# Validate each profraw file before merging — skip corrupt ones.
	local valid_files=()
	local skipped=0
	local total_size=0
	for profraw in "${pgo_dir}"/*.profraw; do
		[ -f "${profraw}" ] || continue
		local fsize
		fsize=$(stat -c%s "${profraw}" 2>/dev/null || stat -f%z "${profraw}" 2>/dev/null || echo 0)
		if [ "${fsize}" -lt 100 ]; then
			log_warning "  Skipping ${profraw##*/}: too small (${fsize} bytes — likely empty/corrupt)"
			skipped=$((skipped + 1))
			continue
		fi
		valid_files+=("${profraw}")
		total_size=$((total_size + fsize))
	done

	if [ "${#valid_files[@]}" -eq 0 ]; then
		log_error "No valid profraw files to merge"
		exit 1
	fi

	local total_size_mb=$((total_size / 1024 / 1024))
	log_info "  Merging ${#valid_files[@]} profraw files (${total_size_mb}MB total, ${skipped} skipped)"

	if "${profdata_tool}" merge -o "${profdata_file}" "${valid_files[@]}" 2>&1 | tail -3; then
		local profdata_size
		profdata_size=$(stat -c%s "${profdata_file}" 2>/dev/null || stat -f%z "${profdata_file}" 2>/dev/null || echo 0)
		local profdata_size_mb=$((profdata_size / 1024 / 1024))
		log_success "  Merged profdata: ${profdata_size_mb}MB (${profdata_file})"
	else
		log_error "  profdata merge failed"
		exit 1
	fi
}

# Stage 1 (build instrumented binary) — extracted for reuse by resume logic.
# Args: pgo_dir instrument_cpu instrument_bin
pgo_stage_run_1() {
	local pgo_dir="$1"
	local instrument_cpu="$2"
	local instrument_bin="$3"

	log_info "Stage 1/3: Building instrumented binary..."
	export COSMOSTRIX_BUILD="nitro-pgo-instrument"
	export COSMOSTRIX_PROFILE="pgo-instrument"
	export COSMOSTRIX_LTO="off"
	export COSMOSTRIX_STRIP="no"
	# Use instrument_cpu (safe target) so the binary can execute on the
	# build host. The final binary (Stage 3) uses final_cpu for shipping.
	export RUSTFLAGS="-C target-cpu=${instrument_cpu} -C profile-generate=${pgo_dir}"

	if ! cargo build --profile pgo-instrument --target "${TARGET}" --jobs "${MAX_JOBS}"; then
		log_error "Stage 1 failed: instrumented build failed"
		exit 1
	fi
	log_success "Stage 1 complete: instrumented binary built"
}

# Stage 3 (build optimized PGO binary) — extracted for reuse by resume logic.
# Args: final_cpu profdata_file nitro_bin
pgo_stage_run_3() {
	local final_cpu="$1"
	local profdata_file="$2"
	local nitro_bin="$3"

	log_info "Stage 3/3: Building PGO-optimized nitro binary..."
	export COSMOSTRIX_BUILD="nitro-pgo"
	export COSMOSTRIX_PROFILE="pgo-use"
	export COSMOSTRIX_LTO="fat"
	export COSMOSTRIX_STRIP="yes"
	export RUSTFLAGS="-C target-cpu=${final_cpu} -C profile-use=${profdata_file}"
	# Append hardened flags (path remap + frame pointers) for the
	# shipping binary. Idempotent — safe to call after RUSTFLAGS is
	# set above. The instrumented binary (Stage 1) is never shipped
	# so it skips hardening.
	apply_hardened_rustflags

	if ! cargo build --profile pgo-use --target "${TARGET}" --jobs "${MAX_JOBS}"; then
		log_error "Stage 3 failed: PGO-optimized build failed"
		exit 1
	fi
	log_success "Stage 3 complete: PGO-optimized binary built"
}

# Check if a stage stamp is valid (exists + commit matches HEAD).
# Args: stamp_file instrument_cpu final_cpu
# Returns 0 if valid, 1 if invalid/missing.
pgo_stage_stamp_valid() {
	local stamp_file="$1"
	local expected_instr_cpu="$2"
	local expected_final_cpu="$3"

	[ -f "${stamp_file}" ] || return 1

	local stamp_commit stamp_instr stamp_final
	stamp_commit=$(grep '^commit=' "${stamp_file}" | cut -d= -f2-)
	stamp_instr=$(grep '^instrument_cpu=' "${stamp_file}" | cut -d= -f2-)
	stamp_final=$(grep '^final_cpu=' "${stamp_file}" | cut -d= -f2-)

	local head
	head=$(git rev-parse HEAD 2>/dev/null || echo "")

	[ "${stamp_commit}" = "${head}" ] || return 1
	[ "${stamp_instr}" = "${expected_instr_cpu}" ] || return 1
	[ "${stamp_final}" = "${expected_final_cpu}" ] || return 1
	return 0
}

# Write a stage stamp file recording commit + CPU targets.
# Args: stamp_file instrument_cpu final_cpu
pgo_stage_stamp_write() {
	local stamp_file="$1"
	local instrument_cpu="$2"
	local final_cpu="$3"

	local head ts_iso
	head=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
	ts_iso=$(date -u +%Y-%m-%dT%H:%M:%SZ)

	cat >"${stamp_file}" <<STAMP_EOF
# PGO stage stamp (generated by scripts/build/build.sh pgo)
commit=${head}
timestamp_iso=${ts_iso}
instrument_cpu=${instrument_cpu}
final_cpu=${final_cpu}
target=${TARGET}
STAMP_EOF
}

# Rotate PGO logs — keep last 5 timestamped logs + always keep workload-N.log
# (overwritten each run).
# Args: logs_dir
pgo_rotate_logs() {
	local logs_dir="$1"
	[ -d "${logs_dir}" ] || return 0

	# List timestamped logs (pgo-YYYYMMDD-HHMMSS.log), oldest first
	local logs=()
	while IFS= read -r f; do
		logs+=("$f")
	done < <(find "${logs_dir}" -maxdepth 1 -name 'pgo-*.log' 2>/dev/null | sort)

	# Keep last 5; delete the rest
	local count="${#logs[@]}"
	if [ "${count}" -gt 5 ]; then
		local to_delete=$((count - 5))
		local i=0
		for f in "${logs[@]}"; do
			if [ "${i}" -lt "${to_delete}" ]; then
				rm -f "${f}"
				i=$((i + 1))
			else
				break
			fi
		done
		log_info "Rotated ${to_delete} old PGO log(s) (kept last 5)"
	fi
}

# Print consolidated PGO summary report.
# Args: nitro_bin instrument_bin pgo_dir profdata_file instrument_cpu final_cpu log_file
pgo_print_summary() {
	local nitro_bin="$1"
	local instrument_bin="$2"
	local pgo_dir="$3"
	local profdata_file="$4"
	local instrument_cpu="$5"
	local final_cpu="$6"
	local log_file="$7"

	local nitro_size instr_size profraw_count profdata_size
	nitro_size=$(du -h "${nitro_bin}" 2>/dev/null | cut -f1 || echo "?")
	instr_size=$(du -h "${instrument_bin}" 2>/dev/null | cut -f1 || echo "?")
	profraw_count=$(find "${pgo_dir}" -name "*.profraw" 2>/dev/null | wc -l)
	if [ -f "${profdata_file}" ]; then
		profdata_size=$(du -h "${profdata_file}" 2>/dev/null | cut -f1 || echo "?")
	else
		profdata_size="(raw dir)"
	fi

	echo ""
	echo "── PGO Build Summary ────────────────────────────────────────────"
	printf "  PGO binary:        %s (%s)\n" "${nitro_bin}" "${nitro_size}"
	printf "  Instrument binary: %s (%s)\n" "${instrument_bin}" "${instr_size}"
	printf "  Profile data:      %s (%s profraw → %s profdata)\n" \
		"${pgo_dir}" "${profraw_count}" "${profdata_size}"
	printf "  Instrument CPU:    %s (must run on host)\n" "${instrument_cpu}"
	printf "  Final CPU:         %s (shipping binary)\n" "${final_cpu}"
	printf "  Training log:      %s\n" "${log_file}"
	echo "────────────────────────────────────────────────────────────────"
}

# Validate PGO gain by building a release baseline + comparing FPS.
# Args: nitro_bin final_cpu
pgo_validate_delta() {
	local nitro_bin="$1"
	local final_cpu="$2"
	local release_bin="target/${TARGET}/release/${PROJECT_NAME}"

	echo ""
	log_step "Delta validation: PGO vs release baseline"

	# 1. Build release baseline (with same final_cpu for fair comparison)
	log_info "Building release baseline (target-cpu=${final_cpu})..."
	export COSMOSTRIX_BUILD="pgo-validate-baseline"
	export COSMOSTRIX_PROFILE="release"
	export COSMOSTRIX_LTO="off"
	export COSMOSTRIX_STRIP="yes"
	export RUSTFLAGS="-C target-cpu=${final_cpu}"
	apply_hardened_rustflags

	if ! cargo build --profile release --target "${TARGET}" --jobs "${MAX_JOBS}"; then
		log_warning "Release baseline build failed — skipping delta validation"
		return 1
	fi
	log_success "Release baseline built"

	# 2. Run --benchmark on both binaries (10s each, same scene)
	local bench_scene="monolith"
	local bench_color="zen"
	local bench_duration=10

	log_info "Benchmarking release baseline (${bench_duration}s, ${bench_scene}/${bench_color})..."
	local release_fps
	release_fps=$("${release_bin}" --benchmark -C "${bench_color}" --bench-io \
		--bench-duration "${bench_duration}" --scene "${bench_scene}" 2>/dev/null |
		grep -oE 'avg_fps[": ]+[0-9.]+' | head -1 | grep -oE '[0-9.]+$' || echo "0")
	log_info "  release avg_fps: ${release_fps}"

	log_info "Benchmarking PGO binary (${bench_duration}s, ${bench_scene}/${bench_color})..."
	local pgo_fps
	pgo_fps=$("${nitro_bin}" --benchmark -C "${bench_color}" --bench-io \
		--bench-duration "${bench_duration}" --scene "${bench_scene}" 2>/dev/null |
		grep -oE 'avg_fps[": ]+[0-9.]+' | head -1 | grep -oE '[0-9.]+$' || echo "0")
	log_info "  pgo avg_fps:     ${pgo_fps}"

	# 3. Compute delta
	if [ "${release_fps}" = "0" ] || [ "${pgo_fps}" = "0" ]; then
		log_warning "Could not extract avg_fps from benchmark output — skipping delta"
		log_info "Run manually: ${nitro_bin} --benchmark"
		return 1
	fi

	local delta_pct
	delta_pct=$(awk -v pgo="${pgo_fps}" -v rel="${release_fps}" \
		'BEGIN { if (rel > 0) printf "%+.1f", (pgo - rel) / rel * 100; else print "N/A" }')

	echo ""
	echo "── PGO Delta Report ─────────────────────────────────────────────"
	printf "  Release baseline: %s avg_fps\n" "${release_fps}"
	printf "  PGO binary:       %s avg_fps\n" "${pgo_fps}"
	printf "  Delta:            %s%%\n" "${delta_pct}"
	echo "────────────────────────────────────────────────────────────────"

	# 4. Regression warning
	if awk -v pgo="${pgo_fps}" -v rel="${release_fps}" \
		'BEGIN { exit !(pgo < rel * 0.98) }'; then
		log_warning "PGO regression detected: PGO is slower than release by more than 2%"
		log_info "This may indicate:"
		log_info "  - Training workloads don't match production usage"
		log_info "  - Profile data is stale (commit changed since training)"
		log_info "  - LLVM version mismatch between instrument + optimize"
		log_info "Try: ./scripts/build/build.sh pgo --no-resume (forces full rebuild)"
	else
		log_success "PGO validation passed: ${delta_pct}% vs release"
	fi
}
