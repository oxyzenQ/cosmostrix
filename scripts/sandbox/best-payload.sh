#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: Linux guest only (inside the kernel-best micro-VM).
#
# The kernel-best payload battery - the deepest E2E the estate owns,
# executed inside the micro-VM on the newest mainline stable kernel
# (the resolver's moving-latest pick) with the best envelope (all
# runner cores, most of its RAM, host-passthrough CPU). The userland
# is an ubuntu:24.04 export with python3, so this lane reuses the
# EXISTING supermassive pty instruments (scripts/supermassive/
# pty_smoke.py + pty_assert.py) instead of a thinner re-implementation:
# the same transport (pty.fork, full byte capture, internal watchdog
# budget) and the same byte assertions the host lane runs. The
# harness appends MASS-RESULT rows to the mass file and exits
# non-zero on any failure, so the legs here verdict on exit codes -
# the same contract supermassive.yml's own steps use.
#
# The three directions this battery proves on the moving-latest
# kernel:
#   1. The full headless surface (floor battery re-run): the info
#      surfaces, --doctor, the --bench-frames headless render
#      contract, the fail-loud error paths, the headless guard -
#      identical instrument to the floor lane, so the two lanes'
#      rows stay comparable (only the kernel differs).
#   2. The pty quit-path depth as root: the --duration deadline
#      exit, the 'q' quit key as live tty input bytes, and the
#      SIGTERM graceful shutdown, each asserting the restore tail
#      (alt-screen leave, mouse/paste/focus disables) the unit suite
#      can only check on synthetic strings.
#   3. The unprivileged direction: the same duration quit-path leg
#      re-run as nobody (setpriv, uid 65534) - the docs' contract
#      that cosmostrix needs no root, executed on a real kernel.
#
# Every leg appends a SANDBOX-RESULT row; the exit code is the
# verdict the init relays as SANDBOX-VERDICT. Pure ASCII only.

set -u

Z=/opt/cosmostrix
BIN="$Z/cosmostrix"
LOG=/tmp
MASS="${LOG}/mass-best.log"
SMOKE="$Z/scripts/supermassive/pty_smoke.py"
ASSERT="$Z/scripts/supermassive/pty_assert.py"
FAILS=0

note() {
	echo "SANDBOX-RESULT: $1 - $2"
}

# One pty leg: the smoke transport, then the byte assertions. The
# harness's own exit codes are the verdict (exit 3 = a failed
# assertion, non-zero = a transport failure); the MASS-RESULT rows
# land in the mass file for the step summary either way. The two
# instruments take disjoint flags (the driver eats --arg/--env/
# --budget/--quit-key/--sigterm, the assert step eats --expect/
# --forbid), so the call form is: run_pty_leg LABEL <smoke args>
# -- <assert args>.
run_pty_leg() {
	_label="$1"
	shift
	local -a smoke=() assert=()
	local sep=0
	local arg
	for arg in "$@"; do
		if [ "$arg" = "--" ]; then
			sep=1
			continue
		fi
		if [ "$sep" -eq 0 ]; then
			smoke+=("$arg")
		else
			assert+=("$arg")
		fi
	done
	if python3 "$SMOKE" --binary "$BIN" \
		--log "${LOG}/best-${_label}.log" --mass "${MASS}" \
		--label "best-${_label}" "${smoke[@]}" &&
		python3 "$ASSERT" --log "${LOG}/best-${_label}.log" \
			--mass "${MASS}" --label "best-${_label}" "${assert[@]}"; then
		note "pty leg best-${_label}" PASS
	else
		note "pty leg best-${_label} (harness or assertion failed)" FAIL
		FAILS=$((FAILS + 1))
	fi
}

# Direction 1 - the full headless surface, the floor battery's own
# instrument (it asserts euid 0; this lane runs it as root too).
echo "SANDBOX: direction 1 - the headless surface (the floor battery's instrument)"
sh "$Z/.sandbox/floor-battery.sh"
_rc=$?
if [ "$_rc" -eq 0 ]; then
	note "headless surface (floor battery re-run)" PASS
else
	note "headless surface (floor battery re-run, rc=$_rc)" FAIL
	FAILS=$((FAILS + 1))
fi

# Direction 2 - the pty quit-path depth, root, real ptys. Budgets
# mirror supermassive.yml (the harness's internal watchdog stays
# narrower than the outer VM timeout).
echo "SANDBOX: direction 2 - the pty quit paths (root)"
run_pty_leg duration-5s \
	--arg=--duration --arg=5 --env=TERM=xterm-256color --budget 25 -- \
	--expect alt-enter --expect alt-leave --expect mouse-off \
	--expect paste-off --expect focus-off --forbid sgr-truecolor

run_pty_leg qkey-5s \
	--env=TERM=xterm-256color --quit-key 5 --budget 25 -- \
	--expect alt-enter --expect alt-leave --expect mouse-off \
	--forbid sgr-truecolor

run_pty_leg sigterm-5s \
	--env=TERM=xterm-256color --sigterm 5 --budget 25 -- \
	--expect alt-leave --expect mouse-off --forbid sgr-truecolor

# Direction 3 - the unprivileged direction on a real kernel: the
# duration quit-path leg re-run as nobody (uid 65534). The docs'
# no-root contract, executed - a renderer that secretly needed root
# would fail here on the pty allocation or the render itself.
echo "SANDBOX: direction 3 - the unprivileged direction (nobody)"
if setpriv --reuid=65534 --regid=65534 --clear-groups \
	python3 "$SMOKE" --binary "$BIN" \
	--log "${LOG}/best-nobody-5s.log" --mass "${MASS}" \
	--label best-nobody-5s --arg=--duration --arg=5 \
	--env=TERM=xterm-256color --budget 25 &&
	python3 "$ASSERT" --log "${LOG}/best-nobody-5s.log" \
		--mass "${MASS}" --label best-nobody-5s \
		--expect alt-enter --expect alt-leave --expect mouse-off \
		--forbid sgr-truecolor; then
	note "pty leg best-nobody-5s (uid 65534)" PASS
else
	note "pty leg best-nobody-5s (uid 65534, setpriv or harness failed)" FAIL
	FAILS=$((FAILS + 1))
fi

# The panic sweep across every leg log (the headless battery keeps
# its own logs under the same /tmp).
if grep -r "panicked at" "${LOG}"/out-*.log "${LOG}"/best-*.log >/dev/null 2>&1; then
	note "panic sweep (no 'panicked at' in any leg log)" FAIL
	FAILS=$((FAILS + 1))
else
	note "panic sweep (no 'panicked at' in any leg log)" PASS
fi

if [ "$FAILS" -eq 0 ]; then
	echo "SANDBOX-BATTERY: all best-lane legs green"
	exit 0
fi
echo "SANDBOX-BATTERY: $FAILS best-lane leg(s) failed"
exit 1
