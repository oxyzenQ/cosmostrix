<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# Security Policy

cosmostrix runs raw-mode on the user's TTY and owns terminal state
while it renders — security is part of the product, not an
afterthought. This file is the entry point: how to report, what
counts as a vulnerability, and where the audit trail lives. The full
posture analysis is told once in [docs/SECURITY_AUDIT.md](docs/SECURITY_AUDIT.md).

## Supported versions

| Version | Branch | Status |
|---------|--------|--------|
| latest | `main` | **supported** — active maintenance (bug fixes, terminal compatibility, hardening) |

Pre-release tags are development artifacts: reports against them are
welcome and triaged, but fixes land on `main` first.

## Reporting a vulnerability

**Do not open a public GitHub issue for security problems.**

Use GitHub's **private vulnerability reporting** on this repository
(*Security tab -> "Report a vulnerability"*), or contact the owner
directly at [with.rezky@gmail.com](mailto:with.rezky@gmail.com).
Both routes reach the maintainer privately.

## What counts as a vulnerability

- Any escape from the terminal-state contract: a crash, hang, or
  degraded terminal the cleanup path cannot restore (the recovery
  tiers are specified in [docs/TERMINAL_KILL_CLEANUP.md](docs/TERMINAL_KILL_CLEANUP.md)).
- Escape-sequence injection through any input surface — the attack
  class the `escape_ctrl` fuzz target exists to gate.
- Config parsing or path handling that reads, writes, or executes
  outside the [safepath whitelist](docs/COSMIC_DRAGON_ARCHITECTURE.md)
  or rejects valid user input unsafely.
- A root/privilege boundary break: root execution is an unsupported
  use case (warning-guarded locally; `--check-update` hard-refused at
  euid 0) — any path that silently escalates or leaks privilege is a
  vulnerability by definition.
- Supply-chain compromise of the build or release artifacts (the
  verification contract is [docs/VERIFY_RELEASE.md](docs/VERIFY_RELEASE.md)).

## Where the audit trail lives

- [docs/SECURITY_AUDIT.md](docs/SECURITY_AUDIT.md) — the posture
  summary (unsafe policy, attack-surface notes, the running-as-root
  analysis and the terminal-ownership threat model).
- [docs/SUPPLY_CHAIN.md](docs/SUPPLY_CHAIN.md) and
  [docs/DEPENDENCY_AUDIT.md](docs/DEPENDENCY_AUDIT.md) — the
  dependency inventory, `cargo-deny` and audit gates, MSRV.
- [docs/VERIFY_RELEASE.md](docs/VERIFY_RELEASE.md) — checksums,
  signatures, and per-artifact verification.
- [CHANGELOG.md](CHANGELOG.md) — the release history records every
  security-relevant fix.
