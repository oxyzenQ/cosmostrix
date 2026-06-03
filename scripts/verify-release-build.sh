# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: MIT

#!/usr/bin/env bash
set -euo pipefail

TARGET="x86_64-unknown-linux-gnu"
BIN_NAME="cosmostrix"
NO_BUILD=0
LOCKED="--locked"

usage() {
	cat <<'EOF'
Usage: scripts/verify-release-build.sh [--no-build] [--unlocked] [profile...]

Profiles default to:
  pro-linux-v1 pro-linux-v2 pro-linux-v3 pro-linux-v4

The script builds Linux x86_64 release variants, prints binary sizes, runs
safe variants with -i, verifies build metadata, and checks stripped status on
Unix. x86-64-v4 is scanned without execution unless the host supports AVX-512F.
EOF
}

log() {
	printf '[INFO] %s\n' "$*"
}

pass() {
	printf '[PASS] %s\n' "$*"
}

fail() {
	printf '[FAIL] %s\n' "$*" >&2
	exit 1
}

have_avx2() {
	if [[ -r /proc/cpuinfo ]]; then
		grep -q '^flags.* avx2' /proc/cpuinfo
	elif command -v sysctl >/dev/null 2>&1; then
		sysctl -a 2>/dev/null | grep -E 'machdep.cpu.features|machdep.cpu.leaf7_features' | grep -qi 'AVX2'
	else
		return 1
	fi
}

have_avx512f() {
	if [[ -r /proc/cpuinfo ]]; then
		grep -q '^flags.* avx512f' /proc/cpuinfo
	elif command -v sysctl >/dev/null 2>&1; then
		sysctl -a 2>/dev/null | grep -E 'machdep.cpu.features|machdep.cpu.leaf7_features' | grep -qi 'AVX512F'
	else
		return 1
	fi
}

build_id_for_profile() {
	case "$1" in
	pro-linux-v1) printf 'linux-x86_64-v1' ;;
	pro-linux-v2) printf 'linux-x86_64-v2' ;;
	pro-linux-v3) printf 'linux-x86_64-v3' ;;
	pro-linux-v4) printf 'linux-x86_64-v4' ;;
	*) fail "Unsupported profile '$1'" ;;
	esac
}

rustflags_for_profile() {
	case "$1" in
	pro-linux-v1) printf -- '-C target-cpu=x86-64' ;;
	pro-linux-v2) printf -- '-C target-cpu=x86-64-v2' ;;
	pro-linux-v3) printf -- '-C target-cpu=x86-64-v3' ;;
	pro-linux-v4) printf -- '-C target-cpu=x86-64-v4' ;;
	*) fail "Unsupported profile '$1'" ;;
	esac
}

baseline_for_profile() {
	case "$1" in
	pro-linux-v1) printf 'x86-64-v1' ;;
	pro-linux-v2) printf 'x86-64-v2' ;;
	pro-linux-v3) printf 'x86-64-v3' ;;
	pro-linux-v4) printf 'x86-64-v4' ;;
	*) fail "Unsupported profile '$1'" ;;
	esac
}

optimization_for_profile() {
	case "$1" in
	pro-linux-v1) printf 'x86-64 baseline (SSE/SSE2)' ;;
	pro-linux-v2) printf 'x86-64-v2 baseline (SSE3/SSSE3/SSE4.1/SSE4.2/POPCNT)' ;;
	pro-linux-v3) printf 'x86-64-v3 baseline (AVX/AVX2/BMI1/BMI2/FMA)' ;;
	pro-linux-v4) printf 'x86-64-v4 baseline (AVX-512)' ;;
	*) fail "Unsupported profile '$1'" ;;
	esac
}

required_features_for_profile() {
	case "$1" in
	pro-linux-v1) printf '' ;;
	pro-linux-v2) printf 'sse4.2 popcnt' ;;
	pro-linux-v3) printf 'avx2 bmi2 fma' ;;
	pro-linux-v4) printf 'avx512f avx512bw avx512cd avx512dq avx512vl' ;;
	*) fail "Unsupported profile '$1'" ;;
	esac
}

denied_features_for_profile() {
	case "$1" in
	pro-linux-v1) printf 'sse4.2 popcnt avx2 bmi2 fma avx512f' ;;
	pro-linux-v2) printf 'avx2 bmi2 fma avx512f' ;;
	*) printf '' ;;
	esac
}

binary_size() {
	local bin="$1"
	if command -v du >/dev/null 2>&1; then
		du -h "$bin" | awk '{print $1}'
	else
		wc -c <"$bin"
	fi
}

scan_binary() {
	local bin="$1"
	local expected="$2"
	local baseline="$3"
	local optimization="$4"
	local required="$5"
	local text

	if command -v strings >/dev/null 2>&1; then
		text="$(strings "$bin")"
	else
		text="$(grep -aE 'linux_x86_64|linux-x86_64|x86-64-v|static optimized build|fat|unwind|yes' "$bin" || true)"
	fi

	grep -Fq "$expected" <<<"$text" || fail "Missing embedded variant '$expected' in $bin"
	grep -Fq "$optimization" <<<"$text" || fail "Missing embedded optimization '$optimization' in $bin"
	grep -Fq "static optimized build" <<<"$text" || fail "Missing embedded dispatch metadata in $bin"
	grep -Fq "$baseline" <<<"$text" || fail "Missing embedded cpu_baseline '$baseline' in $bin"
	grep -Fq "fat" <<<"$text" || fail "Missing embedded lto metadata 'fat' in $bin"
	grep -Fq "unwind" <<<"$text" || fail "Missing embedded panic metadata 'unwind' in $bin"
	grep -Fq "yes" <<<"$text" || fail "Missing embedded strip metadata 'yes' in $bin"
	for feature in $required; do
		grep -Fq "$feature" <<<"$text" || fail "Missing embedded target feature '$feature' in $bin"
	done
	pass "Embedded metadata scan passed for $expected"
}

verify_info_output() {
	local bin="$1"
	local expected="$2"
	local baseline="$3"
	local optimization="$4"
	local required="$5"
	local denied="$6"
	local out

	out="$("$bin" -i)"
	printf '%s\n' "$out"

	grep -Eq "^  variant: ${expected}$" <<<"$out" || fail "Expected variant '$expected' in $bin -i output"
	grep -Fqx "  optimization: ${optimization}" <<<"$out" || fail "Expected optimization: $optimization in $bin -i output"
	grep -Eq '^  dispatch: static optimized build$' <<<"$out" || fail "Expected dispatch 'static optimized build' in $bin -i output"
	grep -Eq "^  cpu_baseline: ${baseline}$" <<<"$out" || fail "Expected cpu_baseline: $baseline in $bin -i output"
	grep -Eq '^  lto: fat$' <<<"$out" || fail "Expected lto: fat in $bin -i output"
	grep -Eq '^  panic: unwind$' <<<"$out" || fail "Expected panic: unwind in $bin -i output"
	grep -Eq '^  strip: yes$' <<<"$out" || fail "Expected strip: yes in $bin -i output"
	for feature in $required; do
		grep -Eq "^  target_features: .*${feature}" <<<"$out" || fail "Expected target feature '$feature' in $bin -i output"
	done
	for feature in $denied; do
		if grep -Eq "^  target_features: .*${feature}" <<<"$out"; then
			fail "Unexpected target feature '$feature' in $bin -i output for $expected"
		fi
	done
	pass "-i metadata passed for $expected"
}

verify_stripped() {
	local bin="$1"

	if [[ "$(uname -s)" != "Linux" && "$(uname -s)" != "Darwin" ]]; then
		log "Skipping stripped check on $(uname -s)"
		return 0
	fi

	if command -v file >/dev/null 2>&1; then
		local desc
		desc="$(file "$bin")"
		printf '%s\n' "$desc"
		grep -qiE 'not stripped' <<<"$desc" && fail "Binary is not stripped according to file: $bin"
	fi

	if command -v readelf >/dev/null 2>&1; then
		if readelf -S "$bin" | grep -Eq '\.(debug|symtab)'; then
			fail "Binary still contains debug/symtab sections: $bin"
		fi
	fi

	pass "Stripped check passed for $bin"
}

profiles=()
while (($#)); do
	case "$1" in
	--help | -h)
		usage
		exit 0
		;;
	--no-build)
		NO_BUILD=1
		shift
		;;
	--unlocked)
		LOCKED=""
		shift
		;;
	*)
		profiles+=("$1")
		shift
		;;
	esac
done

if ((${#profiles[@]} == 0)); then
	profiles=(pro-linux-v1 pro-linux-v2 pro-linux-v3 pro-linux-v4)
fi

for profile in "${profiles[@]}"; do
	expected="$(build_id_for_profile "$profile")"
	baseline="$(baseline_for_profile "$profile")"
	optimization="$(optimization_for_profile "$profile")"
	required="$(required_features_for_profile "$profile")"
	denied="$(denied_features_for_profile "$profile")"
	bin="target/${TARGET}/${profile}/${BIN_NAME}"

	log "Verifying ${profile} (${expected})"

	if ((NO_BUILD == 0)); then
		rustflags="$(rustflags_for_profile "$profile")"
		COSMOSTRIX_BUILD="$expected" \
			COSMOSTRIX_PROFILE="$profile" \
			RUSTFLAGS="$rustflags" \
			cargo build --profile "$profile" --target "$TARGET" ${LOCKED:+$LOCKED}
	fi

	[[ -x "$bin" ]] || fail "Binary not found or not executable: $bin"
	log "Binary size: $(binary_size "$bin")"

	case "$profile" in
	pro-linux-v4)
		if have_avx512f; then
			verify_info_output "$bin" "$expected" "$baseline" "$optimization" "$required" "$denied"
		else
			log "Skipping execution for ${expected}; host lacks AVX-512F"
			scan_binary "$bin" "$expected" "$baseline" "$optimization" "$required"
		fi
		;;
	pro-linux-v3)
		if have_avx2; then
			verify_info_output "$bin" "$expected" "$baseline" "$optimization" "$required" "$denied"
		else
			log "Skipping execution for ${expected}; host lacks AVX2"
			scan_binary "$bin" "$expected" "$baseline" "$optimization" "$required"
		fi
		;;
	*)
		verify_info_output "$bin" "$expected" "$baseline" "$optimization" "$required" "$denied"
		;;
	esac

	verify_stripped "$bin"
done
