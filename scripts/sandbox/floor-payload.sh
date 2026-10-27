#!/bin/sh
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: Linux guest only (inside the kernel-floor micro-VM).
#
# The kernel-floor payload battery - the FULL headless E2E surface
# of the static release binary (pro-linux-amd64-v3-musl, the exact
# shape docs/SYSTEM_REQUIREMENTS.md promises "runs on any Linux
# kernel 2.6.27+"), executed on the REAL floor kernel the floor
# lane boots (2.6.27, the intrepid GA release). POSIX sh on purpose:
# the userland is a busybox skeleton, so every construct here must
# survive busybox ash.
#
# The legs and why they are honest on a tty-less serial console:
#   - The info surfaces (--version, --help, --dump-config, --list-*)
#     and --doctor are documented non-interactive exits: pure rc-0
#     contracts with non-empty stdout.
#   - --bench-frames IS the headless render contract (main.rs: the
#     bench family renders real frames without a tty), so the render
#     engine, the palettes, the charset pipeline and the writer all
#     execute for real - twice, with --no-effects as the effects-off
#     variant.
#   - The error paths pin the fail-loud contracts: an unknown scene
#     value and an unknown flag. The headless-guard leg deliberately
#     does NOT live here: the guest console session carries a
#     controlling terminal (PID 1's console fds) and the era userland
#     ships no setsid to detach it, so the guard could not be
#     exercised in its deterministic ctty-less shape - the contract
#     is proven on the kernel-best lane's setsid leg instead, and in
#     every CI headless run (cron-shaped: no ctty by construction).
#   - --dump-config rides the PIPE form: the v15 security policy
#     refuses a stdout redirected to a file (exit 2, the
#     anti-whitelist-bypass law), and this battery's expect_ok
#     redirects stdout to a log file. Piping is the documented
#     allowed shape (cosmostrix --dump-config | cat), and the pipe
#     end is what the policy probes.
#   - The pty/signal quit-path depth (raw mode, live input bytes,
#     SIGTERM restore tails) deliberately lives on the kernel-best
#     lane's pty harness, whose ubuntu userland can allocate real
#     ptys; this battery's console has none, and pretending
#     otherwise would be a dishonest assertion.
#
# Every leg appends a SANDBOX-RESULT row; the exit code is the
# verdict the init relays as SANDBOX-VERDICT. Pure ASCII only.

set -u

BIN=/opt/cosmostrix/cosmostrix
LOG=/tmp
FAILS=0

note() {
	echo "SANDBOX-RESULT: $1 - $2"
}

# An rc-0, non-empty-stdout leg.
expect_ok() {
	_name="$1"
	shift
	"$@" >"${LOG}/out-${_name}.log" 2>&1
	_rc=$?
	if [ "$_rc" -eq 0 ] && [ -s "${LOG}/out-${_name}.log" ]; then
		note "$_name" PASS
	else
		note "$_name (rc=$_rc, needs rc 0 + non-empty output)" FAIL
		FAILS=$((FAILS + 1))
	fi
}

# A non-zero-exit leg (the fail-loud contract).
expect_fail() {
	_name="$1"
	shift
	"$@" >"${LOG}/out-${_name}.log" 2>&1
	_rc=$?
	if [ "$_rc" -eq 0 ]; then
		note "$_name (expected nonzero exit, got 0)" FAIL
		FAILS=$((FAILS + 1))
	else
		note "$_name" PASS
	fi
}

# The privileged-direction row: the battery runs as root inside the
# disposable VM (cosmostrix needs no root - the best lane proves the
# unprivileged direction; this row only states who ran here).
#
# The uid comes from /proc/self/status, not from the id applet: the
# era userland is intrepid's own busybox-initramfs (1.10.2), which
# ships no id (probed against the real era binary), and read + set --
# word-splitting is carried by every shell in the fleet - ash and
# bash alike. The Uid line's first number is this shell's real uid.
uid=1
while read -r key rest; do
	if [ "$key" = "Uid:" ]; then
		# word splitting is the point: the Uid line's four columns
		# become the positional params, column 1 is the real uid
		# shellcheck disable=SC2086
		set -- $rest
		uid="${1:-1}"
		break
	fi
done </proc/self/status
if [ "$uid" = "0" ]; then
	note "battery identity (uid 0, root, disposable VM)" PASS
else
	note "battery identity (uid $uid, expected root)" FAIL
	FAILS=$((FAILS + 1))
fi

expect_ok version "$BIN" --version
expect_ok help "$BIN" --help
expect_ok dump-config sh -c "$BIN --dump-config | cat"
expect_ok list-scenes "$BIN" --list-scenes
expect_ok list-charsets "$BIN" --list-charsets
expect_ok list-colors "$BIN" --list-colors
expect_ok doctor "$BIN" --doctor
expect_ok bench-frames-30 "$BIN" --bench-frames 30 --screen-size 80x24
expect_ok bench-frames-no-effects "$BIN" --bench-frames 30 --screen-size 80x24 --no-effects

expect_fail scene-unknown-value "$BIN" --scene definitely-not-a-scene
expect_fail flag-unknown "$BIN" --definitely-not-a-flag

# The panic sweep: a Rust panic printed to stderr on any leg is a
# failure even where the exit code survived (a caught-and-exited
# panic still spells "panicked at").
if grep -r "panicked at" "${LOG}"/out-*.log >/dev/null 2>&1; then
	note "panic sweep (no 'panicked at' in any leg log)" FAIL
	FAILS=$((FAILS + 1))
else
	note "panic sweep (no 'panicked at' in any leg log)" PASS
fi

if [ "$FAILS" -eq 0 ]; then
	echo "SANDBOX-BATTERY: all floor legs green"
	exit 0
fi
echo "SANDBOX-BATTERY: $FAILS floor leg(s) failed"
exit 1
