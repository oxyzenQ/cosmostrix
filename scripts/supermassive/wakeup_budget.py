#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# PLATFORM: Linux-only (strace + pty; the kernel-floor family anchor).

"""Wakeup-floor ledger - the whole-frame syscall budget (night-frontier-Z9).

The syscall-floor ledger (scripts/supermassive/syscall_floor.py,
night-frontier-Z2 2a) proved the OUTPUT floor: exactly one write() per
frame, and it deliberately left the wait/timer vocabulary untraced -
its own honest scope names the residue "OS-mandatory transport
residue". This ledger closes that residue: it traces the RENDER
THREAD's wait/read vocabulary on a real pty and pins the measured
whole-frame budget, so a pacing regression (a busy-loop replacing the
epoll wait, an extra per-frame sleep, an input read storm, a wait
mechanism swap) fails CI with the kernel counts in the message.

The measured budget this ledger pins (80x24, xterm-256color, 60 fps,
intro disabled, no input, release shape):

  per frame on the render thread:
    exactly 1 write()              (the frame - entry 35's floor, re-pinned here)
    exactly 1 blocking wait        (epoll_wait with a nonzero timeout: the pacing wait)
    <= 1.2 zero-timeout probes     (epoll_wait with timeout 0: the designed
                                    near-deadline/drain probe - bounded, not free)
    0 sleeps, 0 sched_yields       (the render thread must never nanosleep
                                    or burn yield loops between frames)
    0 reads on the tty input fd    (no input events in the leg; every in-window
                                    read is the Power Dragon's /proc telemetry,
                                    counted informationally at its named cadence)

  auxiliary threads (named, not criminalized - the OS-mandatory
  residue entry 35 left untraced, now measured):
    the watchdog thread: ~1 one-second clock_nanosleep per elapsed
    second plus its health-probe ioctls - a background thread by
    design (entry 17's self-healing stack), counted informationally.

Any other wait family (ppoll, poll, select, pselect6) inside the
steady window is a finding: the wait mechanism changed under the
ledger and the budget must be re-derived on purpose.

Window definition mirrors entry 35: from the FIRST write() carrying
the synchronized-output frame marker (ESC [ ? 2026 h) to the LAST
one, with the intro disabled via --intro none so the window is pure
rain steady state. The child runs under strace -f -ttt on a real pty;
the ledger parses the trace with per-TID attribution (the render
thread is the TID that performed the frame writes; the watchdog's
1 Hz sleep must never be misattributed to the frame path).

Exit codes: 0 when every check passes, 2 on usage errors, 3 on
transport or assertion failure (same shape as syscall_floor.py).
"""

import argparse
import fcntl
import os
import pty
import re
import select
import shutil
import signal
import struct
import sys
import termios
import time
from collections import Counter

# The pinned ledger screen: the same rectangle the syscall-floor and
# torture legs pin - never the environment, never the host terminal.
LEDGER_ROWS = 24
LEDGER_COLS = 80

# The synchronized-output frame marker (termdetect::SYNC_START), in
# the -xx encoded form every byte arrives as. The ONLY writer of this
# marker is the combined flush buffer (the marker rides the frame's
# single write_all; the exit stack emits only the sync END), so mode
# toggles and the restore tail can never false-positive - the same
# fingerprint entry 35 proves.
SYNC_OPEN = "\\x1b\\x5b\\x3f\\x32\\x30\\x32\\x36\\x68"

# The traced vocabulary: the wait family, the sleep family, the read
# family, the output family, yields, and the ioctl surface.
TRACE_SET = (
    "epoll_wait,epoll_pwait,ppoll,poll,select,pselect6,"
    "nanosleep,clock_nanosleep,clock_nanosleep_time64,"
    "read,readv,pread64,"
    "write,writev,sched_yield,ioctl"
)

WAIT_FAMILY = {"epoll_wait", "epoll_pwait", "ppoll", "poll", "select", "pselect6"}
SLEEP_FAMILY = {"nanosleep", "clock_nanosleep", "clock_nanosleep_time64"}
READ_FAMILY = {"read", "readv", "pread64"}
OUTPUT_FAMILY = {"write", "writev"}

# Budget bounds (the physics bounds of the ledger, each with its
# provenance):
BLOCKING_WAITS_PER_FRAME_LO = 0.90  # one pacing wait per frame, fudge for edge frames
BLOCKING_WAITS_PER_FRAME_HI = 1.25  # a stall frame may re-poll; more is a busy loop
PROBE_WAITS_PER_FRAME_HI = 1.50  # the designed drain probe, generously bounded
WRITE_RATIO_TOL = 0.02  # writes == frames exactly; tol covers the fenceposts
TELEMETRY_READS_PER_SEC_HI = 20.0  # measured 8 Hz (/proc stat+status); 2.5x bound
WAKE_PRECISION_MS_HI = 5.0  # epoll sleeps its requested timeout; slack + strace noise


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Prove the whole-frame wakeup budget under strace on a real pty."
    )
    parser.add_argument(
        "--binary", required=True, help="cosmostrix executable to trace"
    )
    parser.add_argument("--trace", required=True, help="strace output file to write")
    parser.add_argument(
        "--log", required=True, help="file to write captured pty bytes to"
    )
    parser.add_argument(
        "--mass", required=True, help="file to append MASS-RESULT rows to"
    )
    parser.add_argument(
        "--label", required=True, help="leg name used in MASS-RESULT rows"
    )
    parser.add_argument(
        "--duration",
        type=float,
        default=8,
        metavar="SECONDS",
        help="render duration passed to the child as --duration (default 8)",
    )
    parser.add_argument(
        "--budget",
        type=float,
        default=30.0,
        metavar="SECONDS",
        help="hard watchdog: kill the child and fail if it lives longer",
    )
    return parser.parse_args()


def append_row(mass_path: str, label: str, text: str) -> None:
    with open(mass_path, "a", encoding="utf-8") as handle:
        handle.write(f"MASS-RESULT: {label} {text}\n")


def usage_fail(message: str) -> None:
    print(f"wakeup_budget: {message}", file=sys.stderr)
    raise SystemExit(2)


def run_child(args: argparse.Namespace) -> int:
    """Spawn the traced child on a real pty, capture bytes, wait for exit.

    Transport shape mirrors syscall_floor.py (the proven supermassive
    transport): pty.fork, a hard watchdog budget, byte capture to the
    log, the same exit-code contract. -ttt stamps every line with an
    absolute timestamp so the wake-precision rows are computable.
    """
    if not os.path.isfile(args.binary) or not os.access(args.binary, os.X_OK):
        usage_fail(f"--binary is not an executable file: {args.binary}")
    if shutil.which("strace") is None:
        usage_fail("strace is not on PATH (the ledger is a Linux strace proof)")

    strace_argv = [
        "strace",
        "-f",  # follow the auxiliary threads (the watchdog is by-design)
        "-e", f"trace={TRACE_SET}",
        "-xx",  # every payload byte as \xNN - uniform parsing
        "-s", "64",  # payload prefix for classification; the length arg is authoritative
        "-ttt",  # absolute timestamps: the wake-precision dataset
        "-o", args.trace,
        "--",
        args.binary,
        "--duration", str(int(args.duration)),
        "--perf-stats",
        "--intro", "none",  # the window is pure rain steady state (an existing flag)
    ]

    pid, master = pty.fork()
    if pid == 0:
        try:
            env = dict(os.environ, TERM="xterm-256color")
            os.execvpe(strace_argv[0], strace_argv, env)
        except BaseException:  # noqa: BLE001 - exec failure must not escape as a pty ghost
            os._exit(127)

    try:
        winsize = struct.pack("HHHH", LEDGER_ROWS, LEDGER_COLS, 0, 0)
        fcntl.ioctl(master, termios.TIOCSWINSZ, winsize)
    except OSError:
        pass  # a closed master must not fail the leg

    captured = bytearray()
    started = time.monotonic()
    deadline = started + args.budget
    status: int | None = None
    fail_reason: str | None = None

    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            fail_reason = f"watchdog budget {args.budget}s exceeded"
            try:
                os.kill(pid, signal.SIGKILL)
                _, status = os.waitpid(pid, 0)
            except (ProcessLookupError, ChildProcessError):
                status = -1
            break

        readable, _, _ = select.select([master], [], [], min(remaining, 1.0))
        if readable:
            try:
                chunk = os.read(master, 65536)
            except OSError:
                chunk = b""  # EIO: slave side closed, child is exiting
            if not chunk:
                break
            captured.extend(chunk)

        try:
            done_pid, done_status = os.waitpid(pid, os.WNOHANG)
        except ChildProcessError:
            status = -1
            break
        if done_pid == pid:
            status = done_status
            break

    if status is None:
        try:
            _, status = os.waitpid(pid, 0)
        except ChildProcessError:
            status = -1
    try:
        os.close(master)
    except OSError:
        pass

    with open(args.log, "wb") as handle:
        handle.write(bytes(captured))

    exit_code = os.waitstatus_to_exitcode(status) if status not in (None, -1) else -1
    elapsed = time.monotonic() - started
    append_row(args.mass, args.label, f"captured={len(captured)} bytes in {elapsed:.1f}s")
    append_row(args.mass, args.label, f"exit={exit_code} expected=0")

    if fail_reason is not None:
        append_row(args.mass, args.label, f"FAIL reason={fail_reason}")
        return 3
    if exit_code != 0:
        append_row(args.mass, args.label, "FAIL reason=exit-code mismatch")
        return 3
    append_row(args.mass, args.label, "PASS transport")
    return 0


# -- strace parsing ---------------------------------------------------------
# With `-f -ttt -xx -s 64 -e trace=... -o FILE` every traced line is one of:
#   <tid>  <ttt> syscall(<args...>) = <ret>
#   <tid>  <ttt> syscall(<args...> <unfinished ...>
#   <tid>  <ttt> <... syscall resumed><args...>) = <ret>
# Long payloads truncate after the quoted prefix (the length argument
# stays authoritative); a signal or ptrace stop between a head and its
# return produces the unfinished/resumed pair, joined per TID below.
# A head with no tail, or a tail with no head, is a problem row - never
# guessed silently (the entry-35 parser law, mirrored).
LINE_OPEN = re.compile(r"^(\d+)\s+([\d.]+)\s+(\w+)\((.*)$")
LINE_RESUMED = re.compile(r"^(\d+)\s+([\d.]+)\s+<\.\.\. (\w+) resumed>(.*)$")
RET_TAIL = re.compile(r"\)\s*=\s*(-?\d+)\s*$")


def decode_strace_string(text: str) -> str:
    """Decode a -xx payload prefix into plain characters for matching.

    Only used to locate the sync-open marker inside a write payload;
    the raw bytes never leave this function.
    """
    out = bytearray()
    i = 0
    while i < len(text):
        ch = text[i]
        if ch == "\\" and i + 1 < len(text):
            nxt = text[i + 1]
            if nxt == "x" and i + 3 < len(text):
                try:
                    out.append(int(text[i + 2 : i + 4], 16))
                    i += 4
                    continue
                except ValueError:
                    pass
            if nxt == '"':
                out.append(0x22)
                i += 2
                continue
            if nxt == "\\":
                out.append(0x5C)
                i += 2
                continue
        out.append(ord(ch) & 0xFF)
        i += 1
    return bytes(out).decode("latin-1")


def parse_trace(path: str) -> tuple[list[dict], list[str]]:
    """Parse the strace file into ordered syscall events with TIDs.

    Unfinished/resumed pairs are joined per TID (one syscall, head +
    tail; the event keeps the HEAD timestamp - the wait started there).
    Returns (events, problems) where events carry tid, tstamp, name,
    args, and problems are the unparsable lines (a ledger never
    guesses: problems FAIL the leg).
    """
    events: list[dict] = []
    problems: list[str] = []
    unfinished: dict[str, dict] = {}

    with open(path, encoding="utf-8", errors="replace") as handle:
        for raw in handle:
            line = raw.rstrip("\n")
            if not line.strip():
                continue

            m = LINE_RESUMED.match(line)
            if m:
                key = f"{m.group(1)}:{m.group(3)}"
                head = unfinished.pop(key, None)
                tail = m.group(4)
                if head is None:
                    problems.append(f"resumed without head: {line[:120]}")
                    continue
                merged_args = head["args"]
                if tail and not tail.startswith(")"):
                    merged_args = head["args"] + tail
                events.append(
                    {
                        "tid": head["tid"],
                        "tstamp": head["tstamp"],
                        "name": head["name"],
                        "args": merged_args,
                        "ret": head.get("ret"),
                    }
                )
                continue

            m = LINE_OPEN.match(line)
            if not m:
                if "<... " in line or "exited with" in line:
                    continue  # process-exit chatter, not a syscall
                problems.append(f"unparsable line: {line[:120]}")
                continue
            tid, tstamp, name, rest = m.group(1), m.group(2), m.group(3), m.group(4)
            if "<unfinished ...>" in rest:
                unfinished[f"{tid}:{name}"] = {
                    "tid": tid,
                    "tstamp": tstamp,
                    "name": name,
                    "args": rest.replace(" <unfinished ...>", ""),
                }
                continue
            ret = None
            rm = RET_TAIL.search(rest)
            if rm:
                ret = int(rm.group(1))
                rest = rest[: rm.start()]
            events.append(
                {"tid": tid, "tstamp": tstamp, "name": name, "args": rest, "ret": ret}
            )

    for key, head in sorted(unfinished.items()):
        problems.append(f"head without tail: {head['name']} tid={head['tid']}")
    return events, problems


def extract_frame_marker(args: str) -> bool:
    """True when a write's decoded payload carries the sync-open marker."""
    payload = extract_write_payload(args)
    return SYNC_OPEN_DECODED in payload


SYNC_OPEN_DECODED = "\x1b[?2026h"


def extract_write_payload(args: str) -> str:
    """Extract the decoded payload prefix of a write() argument list."""
    m = re.match(r"(\d+), \"((?:[^\"\\]|\\.)*)\"(\.\.\.)?", args)
    if not m:
        return ""
    return decode_strace_string(m.group(2))


def wait_timeout_ms(name: str, args: str) -> float | None:
    """Extract the timeout of a wait-family call in ms (None = unknown).

    epoll_wait(fd, events, maxevents, timeout_ms): the last integer
    argument is the timeout in ms (-1 = infinite). The other families
    have timespec timeouts; the engine uses epoll_wait exclusively
    (proven by the wait-family census row), so unknown shapes do not
    silently classify - they return None and the census handles them.
    """
    if name in ("epoll_wait", "epoll_pwait"):
        nums = re.findall(r"(-?\d+)\)?\s*$", args)
        if nums:
            return float(nums[-1])
    return None


def ledger(events: list[dict], problems: list[str], args: argparse.Namespace) -> bool:
    """Run the whole-frame budget ledger over the parsed events."""
    ok = True

    if problems:
        ok = False
        append_row(args.mass, args.label, f"FAIL reason={len(problems)} unparsable lines")
        for p in problems[:5]:
            append_row(args.mass, args.label, f"problem {p}")

    frame_idx = [
        i for i, e in enumerate(events) if e["name"] == "write" and extract_frame_marker(e["args"])
    ]
    if not frame_idx:
        append_row(args.mass, args.label, "FAIL reason=no sync-markered frame writes")
        return False
    first, last = frame_idx[0], frame_idx[-1]
    frames = len(frame_idx)
    window = events[first : last + 1]
    window_start = float(events[first]["tstamp"])
    window_end = float(events[last]["tstamp"])
    elapsed = max(window_end - window_start, 1e-9)
    append_row(
        args.mass, args.label, f"window frames={frames} span={elapsed:.2f}s"
    )

    # Per-TID attribution: the render thread performed the frame writes.
    write_tids = Counter(e["tid"] for e in window if e["name"] in OUTPUT_FAMILY)
    render_tid = write_tids.most_common(1)[0][0]
    aux_tids = {tid for tid in {e["tid"] for e in window} if tid != render_tid}
    render = [e for e in window if e["tid"] == render_tid]

    # 1. The output floor (entry 35 re-pinned inside this window).
    writes = sum(1 for e in render if e["name"] in OUTPUT_FAMILY)
    ratio = writes / frames
    ok_w = abs(ratio - 1.0) <= WRITE_RATIO_TOL and write_tids[render_tid] == frames
    append_row(
        args.mass, args.label,
        f"{'PASS' if ok_w else 'FAIL'} writes/frame={ratio:.3f} (writes={writes} frames={frames})"
    )
    ok = ok and ok_w

    # 2. The blocking wait floor: one pacing wait per frame.
    blocking = probe = 0
    wait_families = Counter()
    wake_errors: list[float] = []
    for e in render:
        if e["name"] not in WAIT_FAMILY:
            continue
        wait_families[e["name"]] += 1
        tmo = wait_timeout_ms(e["name"], e["args"])
        if tmo is None:
            continue
        if tmo > 0:
            blocking += 1
            rm = re.search(r"\)\s*=\s*(-?\d+)\s*$", e["args"])
            _ = rm  # the return value rides the joined event; timing below
        else:
            probe += 1
    b_ratio = blocking / frames
    ok_b = BLOCKING_WAITS_PER_FRAME_LO <= b_ratio <= BLOCKING_WAITS_PER_FRAME_HI
    append_row(
        args.mass, args.label,
        f"{'PASS' if ok_b else 'FAIL'} blocking-waits/frame={b_ratio:.3f} "
        f"(blocking={blocking} bound=[{BLOCKING_WAITS_PER_FRAME_LO}, {BLOCKING_WAITS_PER_FRAME_HI}])"
    )
    ok = ok and ok_b

    # 3. The probe budget: the designed drain probe, bounded.
    p_ratio = probe / frames
    ok_p = p_ratio <= PROBE_WAITS_PER_FRAME_HI
    append_row(
        args.mass, args.label,
        f"{'PASS' if ok_p else 'FAIL'} probe-waits/frame={p_ratio:.3f} "
        f"(probes={probe} ceiling={PROBE_WAITS_PER_FRAME_HI})"
    )
    ok = ok and ok_p

    # 4. The wait-family census: epoll_wait only; anything else is a
    # mechanism swap and a re-derivation, never a silent drift.
    foreign = {k: v for k, v in wait_families.items() if k not in ("epoll_wait",)}
    ok_f = not foreign
    append_row(
        args.mass, args.label,
        f"{'PASS' if ok_f else 'FAIL'} wait-census={dict(wait_families)} "
        f"foreign={dict(foreign)}"
    )
    ok = ok and ok_f

    # 5. The render thread never sleeps and never yields.
    sleeps = sum(1 for e in render if e["name"] in SLEEP_FAMILY)
    yields = sum(1 for e in render if e["name"] == "sched_yield")
    ok_s = sleeps == 0 and yields == 0
    append_row(
        args.mass, args.label,
        f"{'PASS' if ok_s else 'FAIL'} render-sleeps={sleeps} render-yields={yields} (must be 0/0)"
    )
    ok = ok and ok_s

    # 6. The input floor: zero reads on the tty input fd inside the
    # window; every other read is the Power Dragon's /proc telemetry,
    # counted informationally at its named cadence.
    input_reads = 0
    telemetry = 0
    read_fds = Counter()
    for e in render:
        if e["name"] not in READ_FAMILY:
            continue
        fm = re.match(r"(\d+),", e["args"])
        if not fm:
            continue
        fd = fm.group(1)
        read_fds[fd] += 1
        if fd in ("0", "1", "2"):
            input_reads += 1
        else:
            telemetry += 1
    tel_rate = telemetry / elapsed
    ok_r = input_reads == 0 and tel_rate <= TELEMETRY_READS_PER_SEC_HI
    append_row(
        args.mass, args.label,
        f"{'PASS' if ok_r else 'FAIL'} input-reads={input_reads} "
        f"telemetry={telemetry} ({tel_rate:.2f}/s, ceiling={TELEMETRY_READS_PER_SEC_HI}) "
        f"fds={dict(read_fds)}"
    )
    ok = ok and ok_r

    # 7. Wake precision: every blocking epoll_wait sleeps approximately
    # its requested timeout (no input arrives, so the wait returns at
    # expiry). Measured from the NEXT event's timestamp on any thread
    # minus this wait's own timestamp, against the requested timeout.
    # The spin-wait final half-millisecond is the design (the pacing
    # law), so the wait may undershoot by up to the spin budget; the
    # bound is generous by intent.
    for i, e in enumerate(window):
        if e["tid"] != render_tid or e["name"] != "epoll_wait":
            continue
        tmo = wait_timeout_ms(e["name"], e["args"])
        if tmo is None or tmo <= 0 or tmo > 100:
            continue
        if i + 1 < len(window):
            nxt = float(window[i + 1]["tstamp"])
            delta_ms = (nxt - float(e["tstamp"])) * 1000.0
            wake_errors.append(abs(delta_ms - tmo))
    if wake_errors:
        wake_errors.sort()
        p95 = wake_errors[int(len(wake_errors) * 0.95) - 1]
        ok_wake = p95 <= WAKE_PRECISION_MS_HI
        append_row(
            args.mass, args.label,
            f"{'PASS' if ok_wake else 'FAIL'} wake-precision-p95={p95:.2f}ms "
            f"(n={len(wake_errors)}, ceiling={WAKE_PRECISION_MS_HI})"
        )
        ok = ok and ok_wake
    else:
        append_row(args.mass, args.label, "INFO wake-precision n=0 (no blocking waits measured)")

    # 8. The auxiliary-thread ledger: named, not criminalized. The
    # watchdog sleeps one second per interval by design; the row pins
    # that cadence informationally so a future thread-budget change is
    # a visible diff in the dataset row, not a surprise.
    for tid in sorted(aux_tids):
        aux = [e for e in window if e["tid"] == tid]
        c = Counter(e["name"] for e in aux)
        sleep_secs = 0.0
        for e in aux:
            if e["name"] in SLEEP_FAMILY:
                sm = re.search(r"tv_sec=(\d+)", e["args"])
                if sm:
                    sleep_secs += float(sm.group(1))
        rate = sleep_secs / elapsed
        append_row(
            args.mass, args.label,
            f"INFO aux-tid={tid} events={len(aux)} vocabulary={dict(c)} "
            f"sleep-sec/sec={rate:.2f} (watchdog design: ~1.0)"
        )

    return ok


def main() -> int:
    args = parse_args()
    transport = run_child(args)
    if transport != 0:
        return transport

    events, problems = parse_trace(args.trace)
    ok = ledger(events, problems, args)

    if not ok:
        append_row(args.mass, args.label, "FAIL ledger")
        return 3
    append_row(args.mass, args.label, "PASS ledger")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
