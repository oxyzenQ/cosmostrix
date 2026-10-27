#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 rezky_nightky (oxyzenQ)
#
# INSTALL-ON-MY-MACHINE (night-dragon-1): the one-command fast setup
# for the owner's own box. Detects the CPU's x86-64 microarch level,
# builds the release-tier binary with the exact alias the release
# matrix is parity-gated against, and installs it to /usr/bin - the
# one install path, by default, with no flags.
#
# The contract (mirrors the zelynic-family installer law; the
# zejtron reference is scripts/package/install-on-my-machine.sh):
#   1. Linux only, amd64 only (the Linux release shapes are the
#      fully static musl pair - v3 and v4; macOS/FreeBSD/Windows/
#      Termux ship their own archives from the releases page).
#   2. CPU detect: x86-64-v4 if the core carries the AVX-512
#      baseline, else x86-64-v3 if it carries the AVX2 baseline,
#      else refuse - v3/v4 are the only Linux shapes the release
#      matrix builds. The pro-linux-amd64-baseline-musl alias exists
#      but is the kernel-floor lane's shape, deliberately not a
#      release shape (see .cargo/config.toml) - a v1/v2 machine gets
#      the honest pointer instead of a non-shipped tier.
#   3. Build: cargo pro-linux-amd64-v{3,4}-musl - the arch-baseline
#      alias whose rustflags the release matrix pins, so what lands
#      on this machine is the shipped optimization tier. The build
#      is fully static musl and runs as the current user (a root
#      build corrupts target/ ownership).
#   4. Install: /usr/bin/cosmostrix. ONE path, the default, no
#      --system/--user option to choose - the script escalates sudo
#      for the install step ONLY. scripts/package/uninstall.sh knows
#      this path (and ~/.local/bin for manual user installs).
#   5. Config: none written. The binary generates config.new.toml
#      for review-then-rename on its own; no installer-owned config
#      step exists (verified against uninstall.sh's removal list).

set -euo pipefail

cosmostrix="cosmostrix"
REPO_URL="https://github.com/oxyzenQ/cosmostrix"

usage() {
	cat <<EOF
Usage: $0

  Fast setup for this machine: detect the CPU's x86-64 level
  (v3/v4), build the release-tier binary, install to /usr/bin/${cosmostrix}.

  Takes no options - the install path is /usr/bin by law, not a flag.
  Run as your normal user; sudo is escalated internally for the
  /usr/bin install step only.

  Requires: rust (the pinned toolchain via rust-toolchain.toml), the
  musl rust-std target (added automatically), and a Linux on x86-64
  (the build is fully static musl).
EOF
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	-h | --help)
		usage
		exit 0
		;;
	*)
		echo "error: unknown argument: $1 - this script takes no options" >&2
		usage
		exit 2
		;;
	esac
done

# 1. Platform gates: Linux, x86-64, repo root.
if [[ "$(uname -s)" != "Linux" ]]; then
	echo "error: $(uname -s) is not Linux - the Linux release shapes are the amd64 musl pair." >&2
	echo "  macOS/FreeBSD/Windows/Termux: prebuilt archives ship on the releases page:" >&2
	echo "  ${REPO_URL}/releases" >&2
	exit 1
fi
if [[ "$(uname -m)" != "x86_64" ]]; then
	echo "error: $(uname -m) is not amd64 - the Linux release shapes are x86-64-v3/v4 only." >&2
	exit 1
fi
if [[ ! -f Cargo.toml || ! -f .cargo/config.toml ]]; then
	echo "error: run this script from the cosmostrix repo root." >&2
	exit 1
fi

# Refuse to run as root - the build step must run as the current user
# (a root build creates root-owned files in target/, breaking every
# future cargo clean/build for the normal user). sudo is escalated
# internally for the /usr/bin install step only.
if [[ $EUID -eq 0 ]]; then
	echo "error: do not run this script with sudo." >&2
	echo "  the build step would run as root, corrupting target/ ownership." >&2
	echo "  Run: $0   (sudo is used internally only for /usr/bin)" >&2
	exit 1
fi

# 2. CPU detect: the x86-64 microarch level from /proc/cpuinfo flags.
flags="$(grep -m1 '^flags' /proc/cpuinfo)"
detect_level() {
	# v4 baseline: AVX-512 Foundation + BW + CD + DQ + VL.
	if [[ "$flags" =~ avx512f ]] && [[ "$flags" =~ avx512bw ]] &&
		[[ "$flags" =~ avx512cd ]] && [[ "$flags" =~ avx512dq ]] &&
		[[ "$flags" =~ avx512vl ]]; then
		echo "v4"
		return 0
	fi
	# v3 baseline: AVX2 + BMI2 + FMA (the load-bearing trio), plus
	# the rest of the level's required surface where present.
	if [[ "$flags" =~ avx2 ]] && [[ "$flags" =~ bmi2 ]] &&
		[[ "$flags" =~ fma ]] && [[ "$flags" =~ sse4_2 ]] &&
		[[ "$flags" =~ ssse3 ]] && [[ "$flags" =~ avx ]] &&
		[[ "$flags" =~ bmi1 ]] && [[ "$flags" =~ movbe ]]; then
		echo "v3"
		return 0
	fi
	return 1
}
LEVEL="$(detect_level)" || {
	echo "error: this core is below the x86-64-v3 baseline." >&2
	echo "  the Linux release shapes are v3/v4 only - the baseline musl" >&2
	echo "  alias exists for the kernel-floor lane, not as a release tier." >&2
	echo "  On a v1/v2 machine build the baseline explicitly instead:" >&2
	echo "  cargo pro-linux-amd64-baseline-musl   (see .cargo/config.toml)" >&2
	exit 1
}
echo ">> [1/4] CPU level detected: x86-64-${LEVEL}"

ALIAS="pro-linux-amd64-${LEVEL}-musl"

# The alias builds for the musl triple: its rust-std target must be
# installed (idempotent; the alias links through rust-lld, so the
# target is the only prerequisite - see .cargo/config.toml).
if ! rustup target list --installed 2>/dev/null | grep -q '^x86_64-unknown-linux-musl$'; then
	echo ">> Adding the musl rust-std target (rustup target add)"
	rustup target add x86_64-unknown-linux-musl
fi

# 3. Build: the release-tier alias (the matrix's own shape).
echo ">> [2/4] Building the release tier: cargo ${ALIAS}"
cargo "${ALIAS}"
BINARY="target/x86_64-unknown-linux-musl/${ALIAS}/${cosmostrix}"
if [[ ! -f "${BINARY}" ]]; then
	# The musl alias builds into target/x86_64-unknown-linux-musl/<alias>/;
	# fall back to a find so a profile rename upstream cannot strand
	# the installer.
	BINARY="$(find target -maxdepth 3 -type f -name "${cosmostrix}" -perm -u+x | head -1 || true)"
fi
if [[ -z "${BINARY}" || ! -f "${BINARY}" ]]; then
	echo "error: build produced no ${cosmostrix} binary under target/." >&2
	exit 1
fi
echo "   built: ${BINARY}"

# 4. Verify: the official-build signature string every locally built
# binary carries (README: the embedded 'Cosmic Dragon — Official
# Build by rezky_nightky (oxyzenQ)' marker) - a direct raw-byte search
# of the artifact, not a strings(1) pipe. Why: strings lives in
# binutils, which is NOT a prerequisite of this script (the musl
# aliases link self-contained via the toolchain's own musl runtime and
# lld, so a box can build the release tier without ever installing
# binutils) - and the old check swallowed a missing strings
# (2>/dev/null) into a false 'refusing to install'. grep is already a
# hard dependency (the CPU detect reads /proc/cpuinfo through it), so
# this check adds zero new tools and cannot false-positive on a
# missing one.
if ! grep -aFq "Official Build by rezky_nightky" "${BINARY}"; then
	echo "error: the built binary does not carry the official-build" >&2
	echo "  signature string - the build environment or the source tree" >&2
	echo "  is not what the release law expects. Refusing to install." >&2
	exit 1
fi
echo ">> [3/4] Installing to /usr/bin/${cosmostrix} (sudo for this step only)"
sudo install -Dm755 "${BINARY}" "/usr/bin/${cosmostrix}"
echo "   installed: /usr/bin/${cosmostrix}"

# 5. Verify from the installed path.
echo ">> [4/4] Verify"
/usr/bin/"${cosmostrix}" -V
if ! command -v "${cosmostrix}" >/dev/null 2>&1; then
	echo "note: /usr/bin is not on this shell's PATH yet - open a new shell." >&2
fi
echo
echo "Config: none written - the binary generates config.new.toml on"
echo "  first run (review it, then rename to config.toml). Run"
echo "  ${cosmostrix} --doctor for the health battery."
echo
echo "Remove later: scripts/package/uninstall.sh"
echo "Next: ${cosmostrix} --help   |   Docs: ${REPO_URL}#readme"
