<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

# Night Frontier Z5a — The Paste Fuse: the structural wedge kill

Owner: oxyzenQ | Session: night-frontier-Z5 | Date: 2026-11-19
Reference: github.com/oxyzenQ/zelynic (mature infra lineage)

## The finding this work closes

`docs/FUTURE_BACKLOG.md` section 5a (night-dragon-maintenance-5) recorded
the one open hostile-input finding: a dangling bracketed-paste START
marker (`ESC [200~` with no `ESC [201~` end) wedged the input path —
every later byte, the `q` quit key included, was swallowed as paste
content until the torture watchdog SIGKILLed the session (rc=-9, first
CI evidence run 40466357873's torture leg). The parked decision named
two app-side defenses: cap/drain the paste buffer, or time out
unterminated paste sequences.

## Why the parked options were not implementable (verified, not assumed)

The paste buffer is not cosmostrix's. Reading the actual crossterm 0.29
source in the registry (`src/event/source/unix/tty.rs`,
`src/event/sys/unix/parse.rs`):

- `Parser::advance` pushes every byte into `self.buffer` (an unbounded
  `Vec<u8>`); `parse_event` returns `Ok(None)` for an incomplete
  bracketed paste, so the buffer is retained; `parse_csi_bracketed_paste`
  only yields the event once the buffer ends with `ESC [201~`. No cap,
  no timeout, no public drain, no parser-state accessor.
- The event pump (`terminal/mod.rs` `poll_event`/`read_event`) is pure
  crossterm; cosmostrix never sees raw bytes and cannot bound, drain,
  or detect the wedge state from its side. An absence-of-events detector
  is unsound (an idle session looks identical), and an fd-readability
  probe is racy (crossterm drains the tty queue into its own buffer).
- Patching the crate would be a perma-fork of a core dependency —
  rejected under the dependency law (the skin may be upgraded, the
  heart is never edited).

## The structural fix (compiler-enforced, not mitigated)

The wedge state requires the parser's paste-accumulation branch. That
branch is `#[cfg(feature = "bracketed-paste")]` in crossterm. With the
feature compiled out:

- `ESC [200~` falls through to the ordinary CSI path:
  `parse_csi_special_key_code` parses the parameter `200`, finds no
  matching key code, and returns `Err` — `Parser::advance` clears the
  buffer and continues. The marker is inert; nothing accumulates.
- The `Event::Paste` variant no longer exists in the type system, so a
  paste-handling arm cannot even be written (the compiler enforces the
  removal).

The shipped changes, one surface each:

1. `Cargo.toml` — the `bracketed-paste` feature removed from crossterm
   (the rationale lives in the dependency comment).
2. `terminal/mod.rs` — `EnableBracketedPaste` never sent; the
   `bracketed_paste_enabled` bookkeeping flag deleted.
3. `terminal/cleanup.rs`, `terminal/restore.rs` — the conditional
   `DisableBracketedPaste` commands deleted. The raw
   `TERMINAL_RESTORE_SEQUENCE` / `TERMINAL_RESET_SEQUENCE` still carry
   `ESC [?2004l` on purpose: a baseline reset that disarms paste mode a
   previous process (shell, multiplexer) may have armed. The
   `paste-off` restore-tail marker (`pty_assert.py`) stays valid.
4. `interactive/input.rs` — the `PasteBurstGuard` now arms itself by
   burst rate: three plain-printable Presses inside 50 ms are the
   terminal paste signature (far above human typing, far below a real
   paste burst); auto-Repeat never counts (a held key rides one press).
   Suppressed presses extend the window — the original suppression law,
   unchanged.
5. `interactive/event_loop.rs` — the per-frame event drain is
   wall-clock bounded (2 ms). This closes the neighboring unboundedness
   the wedge investigation surfaced: a hostile stream of COMPLETE junk
   sequences (mouse-report bytes, key codes) keeps crossterm's event
   queue perpetually non-empty, and an unbounded drain would spin one
   frame forever at 100% CPU. Overflow stays queued in crossterm and
   drains over the following frames; a real multi-thousand-event paste
   still fits the budget.

## Honest residual (documented, not hidden)

- The first two characters of a paste pass through before the burst
  window arms (a paste literally starting with `q` could quit). Before
  Z5a: bracketed-paste terminals had zero live head characters, but
  every terminal WITHOUT bracketed-paste support (Windows conhost,
  older VTE) had ALL paste characters live. Two live head characters is
  strictly closer to the safe end of that range, and pasted content was
  always discarded (no text input exists in cosmostrix).
- The wider unterminated-CSI class (an endless `ESC [1;2;3...` digit
  stream) still accumulates inside crossterm — upstream behavior,
  outside this repo's control without a fork. With the bounded drain,
  the impact is bounded work per frame; the input path resumes the
  moment the corrupt stream stops (the next non-parameter byte closes
  the pending sequence, and the byte that closes it is consumed — one
  extra keypress may be eaten once, then input is fully live).
- Slow pastes (< 60 chars/s, e.g. a huge blob over a slow link) never
  cross the burst signature; their characters behave like typed keys.
  Identical to the pre-Z5a behavior on every non-bracketed terminal.

## The proof

- Unit: `test/interactive/tests.rs` — the guard's burst arming,
  suppression, expiry, human-typing pass-through, auto-Repeat immunity,
  and shortcut-letter immunity are all asserted (the
  `note_bracketed_paste` API and its tests are gone with the event).
- Torture: `scripts/supermassive/torture.py` gains the dedicated
  `paste-wedge` leg — a bare `ESC [200~` plus hostile content, then the
  flood leg's proven `xq` retry timeline; the session must quit via `q`
  (rc=0) with the full restore tail. The input-flood pool regains the
  dangling START marker as a regression canary (it was removed in
  maintenance-5 because the wedge made the leg flaky; the structural
  fix makes it junk again).
- A/B: the 10 s A/B campaign (visual density-gini / frame-entropy,
  perf fps / dirty cells) must show the steady state unchanged — the
  fuse touches the input path only, and benches carry no paste traffic.

<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying on
  any specific number (test count, LOC, FPS, ms timeout), file path,
  function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
