#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
#
# Pre-commit gatekeeper script for cosmostrix.
# Runs all non-code linters/checks before allowing a commit.
#
# Usage:
#   ./scripts/gates/gate-keepers.sh           # Run all checks
#   ./scripts/gates/gate-keepers.sh --fix    # Run with auto-fix where possible
#
# Checks performed (exclude Rust core code — use `cargo clippy` for that):
#   1.  Shell scripts (strict triad):
#         1a. bash -n   — syntax check (fast fail-fast pre-filter)
#         1b. shellcheck — static analysis (default rule set)
#         1c. shfmt -d  — canonical formatting (tabs, function braces
#             on own line, case branches expanded); --fix-all runs
#             `shfmt -w` to auto-canonicalize
#   2.  yamllint on all .yml/.yaml files
#   3.  actionlint on all .github/workflows/*.yml
#   4.  TOML syntax validation (python3 tomllib)
#   5.  markdownlint on all .md files
#   6.  codespell on all text files
#   7.  SPDX license header check
#   8.  LOC guard (800-line hard cap on .rs files, per src/RULES_LOC.md)
#   9.  Rust version sync check
#  10.  Documentation disclaimer check (all .md files have the
#       "source code is truth, cross-check before relying" disclaimer)
#  11.  Symbol-only output guard (v80.0.0-beta.2 owner rule — no icon
#       glyphs anywhere in src/test/scripts output surfaces; ASCII
#       symbols only: "!" = warning, "OK"/"+" = pass, "X"/"-" = fail;
#       test/ joined the scan scope in NIGHT-hunter-5)
#  12.  Comment style guard (2026-09-04 owner rule — no decorative
#       markdown emphasis, bold/italic asterisk markers, in any comment
#       type; comments are plain prose, see docs/COMMENT_STYLE.md;
#       covers src/ AND the mirrored test/ tree since NIGHT-hunter-5)
#  13.  Language audit (2026-09-11 owner rule, NIGHT-depthtest-2 —
#       pure English: commit messages, comments, strings, docs and
#       diagnostics; scripts/audit/language_audit.py classifies functional
#       glyph data, math notation and unicode-stress fixtures as kept)
#  14.  File permission guard (2026-09-13 owner rule — git-tracked
#       files 644, tracked executables and directories 755, shebang
#       parity; scripts/gates/check-permissions.sh, --fix-all chmods)
#  15.  Emoji sweep (2026-09-14 owner rule, NIGHT-hunt-48 — repo-wide:
#       no emoji-class codepoints in ANY tracked text file, not just
#       docs; strict detector scripts/audit/emoji-audit.py, exit 1 on hits;
#       fail classes mirror the RULES.md Output Glyph Policy blocks)
#  16.  CI path-filter hygiene (2026-09-24 owner rule, NIGHT-boost-1 —
#       workflow paths/paths-ignore entries that point inside a
#       directory must be globs ('scripts/**'), never hardcoded
#       filenames ('scripts/example.sh') that rot on rename;
#       scripts/gates/check-ci-path-filters.py, root-level files exempt)
#  17.  Scripts LOC guard (2026-09-24 owner rule, NIGHT-lts-1 — every
#       .sh/.py under scripts/ at any depth stays at or below 1000
#       gross lines; scripts/gates/check-scripts-loc.sh mirrors check 8's
#       marker-exemption semantics for shell and Python)
#  18.  Name-case rule (owner mandate, NIGHT-dinner-2 —
#       scripts/gates/check-name-case.py: the project name is
#       lowercase cosmostrix in every context, paths included; the
#       uppercase survivors are the identifier family (COSMOSTRIX_*
#       environment variables, the COSMOSTRIX-* markers), the
#       all-caps banner titles heading the scripts/ gate files, and
#       the all-caps display banners the binary prints. Every
#       tracked file and path is scanned — nothing excluded, the
#       archive included — and any other casing fails the push.
#       (Supersedes the old section 6c naming check, which caught
#       one casing, exempted the archive, and never looked at
#       paths.)
#  19.  Release-notes shape contract (zelynic NIGHT-dinner-26
#       lineage, dinner-11 — scripts/gates/check-release-notes-
#       shapes.sh): the release-body generator is re-executed
#       through its real positional CLI against a hermetic temp
#       git repo for every documented invocation shape — stable
#       (PREV == LAST_STABLE, single range), pre-release (WARNING
#       alert), dual range (both distances), initial release
#       (present-but-empty optional args, the legal-empty class
#       zelynic's v11.0.0 stable publish died on), and the
#       missing-argument usage error.
#
# Exit codes:
#   0 = all checks passed
#   1 = one or more checks failed

set -euo pipefail

# ── Colors ─────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PASS=0
FAIL=0
FIX_MODE=false

if [[ "${1:-}" == "--fix" || "${1:-}" == "--fix-all" ]]; then
	FIX_MODE=true
fi

info() { echo -e "${GREEN}[PASS]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
fail() {
	echo -e "${RED}[FAIL]${NC} $1"
	FAIL=$((FAIL + 1))
}
header() {
	echo ""
	echo "── $1 ──"
}

# ── 1. Shell scripts (strict triad: bash -n + shellcheck + shfmt -d) ───────
# Resolve the .sh file list once and reuse across the three sub-checks.
# Excludes .git and target/ trees; .git is repo metadata, target/ is build
# output (vendor-generated scripts there are not our concern).
SHELL_FILES=$(find . -name '*.sh' -not -path './.git/*' -not -path './target/*' 2>/dev/null)

# ── 1a. bash -n (syntax check) ─────────────────────────────────────────────
# Fast-fail pre-filter: if bash itself rejects the syntax, there is no
# point running shellcheck or shfmt — the file is not parseable. This
# catches unbalanced quotes/braces/heredocs in milliseconds, before the
# slower static-analysis tools even start.
header "bash -n (syntax)"
if [ -n "$SHELL_FILES" ]; then
	BASHN_ERR=0
	# shellcheck disable=SC2086 # word splitting is intentional for file list
	for f in $SHELL_FILES; do
		if ! bash -n "$f" 2>&1; then
			fail "bash -n: syntax error in $f"
			BASHN_ERR=$((BASHN_ERR + 1))
		fi
	done
	if [ "$BASHN_ERR" -eq 0 ]; then
		info "bash -n: all .sh files syntax-clean"
		PASS=$((PASS + 1))
	fi
else
	info "bash -n: no .sh files found"
	PASS=$((PASS + 1))
fi

# ── 1b. shellcheck (static analysis) ───────────────────────────────────────
header "shellcheck"
if command -v shellcheck >/dev/null 2>&1; then
	if [ -n "$SHELL_FILES" ]; then
		# shellcheck disable=SC2086 # word splitting is intentional for file list
		if shellcheck ${SHELL_FILES} 2>&1; then
			info "shellcheck: all .sh files pass"
			PASS=$((PASS + 1))
		else
			fail "shellcheck: errors found in .sh files"
		fi
	else
		info "shellcheck: no .sh files found"
		PASS=$((PASS + 1))
	fi
else
	warn "shellcheck not installed — skipping"
fi

# ── 1c. shfmt -d (format check) ────────────────────────────────────────────
# Canonical style is shfmt's default: tab indent, function braces on
# their own line, case branches expanded. --fix-all runs `shfmt -w` to
# auto-canonicalize; the diff is then empty on the next run.
header "shfmt -d (format)"
if command -v shfmt >/dev/null 2>&1; then
	if [ -n "$SHELL_FILES" ]; then
		# shellcheck disable=SC2086 # word splitting is intentional for file list
		if shfmt -d ${SHELL_FILES} 2>&1; then
			info "shfmt: all .sh files formatted"
			PASS=$((PASS + 1))
		else
			if $FIX_MODE; then
				# shellcheck disable=SC2086 # word splitting is intentional for file list
				if shfmt -w ${SHELL_FILES} 2>&1; then
					info "shfmt: auto-canonicalized (review $(git diff))"
					PASS=$((PASS + 1))
				else
					fail "shfmt: auto-format failed (review errors above)"
				fi
			else
				fail "shfmt: .sh files not formatted (run with --fix-all to auto-format)"
			fi
		fi
	else
		info "shfmt: no .sh files found"
		PASS=$((PASS + 1))
	fi
else
	warn "shfmt not installed — skipping (https://github.com/mvdan/sh)"
fi

# ── 2. Yamllint ────────────────────────────────────────────────────────────
header "Yamllint"
if command -v yamllint >/dev/null 2>&1; then
	# CI parity (2026-08-30 lesson): .github/** must pass the repo
	# .yamllint config — the same one workflow-ci.yml enforces
	# (line-length max 200 included). The old relaxed-only inline
	# config hid a 216-char line in crates-io.yml for three pushes.
	GITHUB_YAML=$(find .github -name '*.yml' -o -name '*.yaml' 2>/dev/null)
	OTHER_YAML=$(find aur .cargo -name '*.yml' -o -name '*.yaml' 2>/dev/null)
	YAML_OK=0
	if [ -n "$GITHUB_YAML" ]; then
		# shellcheck disable=SC2086 # word splitting is intentional for file list
		yamllint -c .yamllint ${GITHUB_YAML} 2>&1 || YAML_OK=1
	fi
	if [ -n "$OTHER_YAML" ]; then
		# aur/.cargo are not linted by CI; keep the relaxed inline
		# config for them.
		# shellcheck disable=SC2086 # word splitting is intentional for file list
		yamllint -d "{extends: default, rules: {line-length: disable, document-start: disable, truthy: disable}}" ${OTHER_YAML} 2>&1 || YAML_OK=1
	fi
	if [ "$YAML_OK" -eq 0 ]; then
		info "yamllint: all YAML files pass (.github under repo config, aur/.cargo relaxed)"
		PASS=$((PASS + 1))
	else
		fail "yamllint: errors found in YAML files"
	fi
else
	warn "yamllint not installed — skipping"
fi

# ── 3. Actionlint ──────────────────────────────────────────────────────────
header "Actionlint"
if command -v actionlint >/dev/null 2>&1; then
	if actionlint .github/workflows/*.yml 2>&1; then
		info "actionlint: all workflow files pass"
		PASS=$((PASS + 1))
	else
		fail "actionlint: errors found in workflow files"
	fi
else
	warn "actionlint not installed — skipping"
fi

# ── 4. TOML Syntax ─────────────────────────────────────────────────────────
header "TOML Syntax"
if command -v python3 >/dev/null 2>&1; then
	TOML_ERR=0
	while IFS= read -r -d '' f; do
		if ! python3 -c "import tomllib, sys; tomllib.load(open(sys.argv[1], 'rb'))" "$f" 2>/dev/null; then
			echo -e "${RED}INVALID TOML: ${f}${NC}"
			TOML_ERR=$((TOML_ERR + 1))
		fi
	done < <(find . -name '*.toml' -not -path './target/*' -not -path './.git/*' -print0 2>/dev/null)
	if [ "$TOML_ERR" -eq 0 ]; then
		info "TOML: all .toml files valid"
		PASS=$((PASS + 1))
	else
		fail "TOML: ${TOML_ERR} file(s) have syntax errors"
	fi
else
	warn "python3 not installed — skipping"
fi

# ── 5. Markdownlint ────────────────────────────────────────────────────────
header "Markdownlint"
if command -v markdownlint >/dev/null 2>&1 || command -v npx >/dev/null 2>&1; then
	MD_LINT="markdownlint"
	if ! command -v markdownlint >/dev/null 2>&1; then
		MD_LINT="npx --yes markdownlint-cli"
	fi
	# shellcheck disable=SC2086 # MD_LINT may contain spaces (npx --yes ...)
	if $MD_LINT --config .markdownlint.yaml '**/*.md' --ignore 'docs/archive/**' --ignore 'target/**' --ignore '.git/**' 2>&1; then
		info "markdownlint: all .md files pass"
		PASS=$((PASS + 1))
	else
		if $FIX_MODE; then
			warn "markdownlint: auto-fixing..."
			# shellcheck disable=SC2086 # MD_LINT may contain spaces
			$MD_LINT --fix --config .markdownlint.yaml '**/*.md' --ignore 'docs/archive/**' --ignore 'target/**' --ignore '.git/**' 2>&1 || true
			info "markdownlint: fixed (review changes)"
			PASS=$((PASS + 1))
		else
			fail "markdownlint: errors found in .md files (run with --fix to auto-fix)"
		fi
	fi
else
	warn "markdownlint not installed — skipping"
fi

# ── 6. Codespell ──────────────────────────────────────────────────────────
header "Codespell"
if command -v codespell >/dev/null 2>&1; then
	if codespell --config .codespellrc . --skip '.git,target,*.lock,Cargo.lock' 2>&1; then
		info "codespell: no spelling errors"
		PASS=$((PASS + 1))
	else
		# Deliberately NOT auto-fixed even under --fix-all: codespell -w
		# would rewrite identifiers, ASCII art, and URLs where apparent
		# misspellings are intentional (see .codespellrc ignore history).
		if $FIX_MODE; then
			fail "codespell: spelling errors found (never auto-fixed - review manually)"
		else
			fail "codespell: spelling errors found"
		fi
	fi
else
	warn "codespell not installed — skipping"
fi

# ── 6b. Python lint (ruff) ─────────────────────────────────────────────────
# Parity with the CI job "Project lint (codespell + ruff)"
# (.github/workflows/ci.yml -> project_lint). Before this check existed,
# python files passed gate-keepers locally but failed that CI job - the
# gatekeeper was not a faithful pre-commit proxy. Runs the exact two
# commands the CI job runs. NIGHT-hunt-47-depthbore: the glob widened
# from maxdepth 1 to the whole scripts/ tree (recursive, mirroring the
# .sh convention above) so scripts/depthbore/*.py cannot escape the
# gate just by living in a subdirectory.
header "Python lint (ruff)"
PY_FILES=$(find scripts -name '*.py' -not -path '*/target/*' 2>/dev/null)
if [ -n "$PY_FILES" ]; then
	if command -v ruff >/dev/null 2>&1; then
		RUFF_OK=0
		if $FIX_MODE; then
			# Auto-fix: apply lint fixes (safe rules only) + format.
			# shellcheck disable=SC2086 # word splitting is intentional for file list
			if ! ruff check --fix ${PY_FILES} 2>&1; then
				fail "ruff check: unfixable python lint errors remain (fix manually)"
				RUFF_OK=1
			fi
			# shellcheck disable=SC2086 # word splitting is intentional for file list
			ruff format ${PY_FILES} 2>&1 || true
		else
			# shellcheck disable=SC2086 # word splitting is intentional for file list
			if ! ruff check ${PY_FILES} 2>&1; then
				fail "ruff check: python lint errors found (auto-fixable via --fix-all)"
				RUFF_OK=1
			fi
			# shellcheck disable=SC2086 # word splitting is intentional for file list
			if ! ruff format --check ${PY_FILES} 2>&1; then
				fail "ruff format: python files not formatted (auto-fixable via --fix-all)"
				RUFF_OK=1
			fi
		fi
		# CI parity guard: ruff's EXE001 flags shebang'd files that lack the
		# executable bit. Same rule, checked locally so the gatekeeper
		# catches it before CI does (incident run #1484).
		# shellcheck disable=SC2086 # word splitting is intentional for file list
		for py in ${PY_FILES}; do
			if head -n 1 "$py" | grep -q '^#!' && [ ! -x "$py" ]; then
				if $FIX_MODE; then
					chmod +x "$py"
					echo "  fixed: chmod +x $py (EXE001)"
				else
					fail "ruff EXE001 parity: $py has a shebang but is not executable (auto-fixable via --fix-all)"
					RUFF_OK=1
				fi
			fi
		done
		if [ "$RUFF_OK" -eq 0 ]; then
			info "ruff: all python files lint-clean and formatted"
			PASS=$((PASS + 1))
		fi
	else
		warn "ruff not installed — skipping (pip install ruff, or fetch the static binary from https://github.com/astral-sh/ruff/releases)"
	fi
else
	info "ruff: no .py files found"
	PASS=$((PASS + 1))
fi

# ── 7. SPDX License Header Check ──────────────────────────────────────────
header "SPDX License Headers"
if [ -f scripts/gates/check-headers.sh ]; then
	if bash scripts/gates/check-headers.sh 2>&1; then
		info "SPDX headers: all files have license headers"
		PASS=$((PASS + 1))
	else
		if $FIX_MODE; then
			# No auto-injector for SPDX headers exists (deliberate: the
			# correct header text varies by file type, and a wrong header
			# is worse than a missing one). The check output above lists
			# exactly which files need the two-line header:
			#   # Copyright (C) 2026 rezky_nightky
			#   # SPDX-License-Identifier: GPL-3.0-only
			fail "SPDX headers: some files missing license headers (no auto-fix - add the 2-line header listed above)"
		else
			fail "SPDX headers: some files missing license headers"
		fi
	fi
else
	warn "check-headers.sh not found — skipping"
fi

# ── 8. LOC Guard (800-line hard cap) ───────────────────────────────────────
# NIGHT-enhanced-hunt-F: capture the full check-rs-loc.sh output and
# only surface it when the check FAILS. The previous `| tail -3` pipe
# printed the "OK (with migration debt): N file(s) exceed 800" success-
# path summary even when there were zero violations — noise that
# contradicts the gate-keepers "only show what matters" principle.
# On failure, print the full output (not just tail -3) so the
# contributor sees the VIOLATES lines AND the FAIL summary block.
header "LOC Guard"
if [ -f scripts/gates/check-rs-loc.sh ]; then
	# NIGHT-lts-1 fix: the previous bare `LOC_OUTPUT=$(...)` capture
	# at top level under `set -euo pipefail` let errexit kill the
	# gatekeeper silently on a violation - the else-branch below (the
	# NIGHT-enhanced-hunt-F full-output surface) was dead code and the
	# COMMIT BLOCKED summary never printed. Capturing inside the if
	# test position exempts the assignment from errexit, so a real
	# violation now prints the per-file VIOLATES lines and the FAIL
	# summary before the exit.
	if LOC_OUTPUT=$(bash scripts/gates/check-rs-loc.sh 2>&1); then
		info "LOC guard: all .rs files ≤800 lines"
		PASS=$((PASS + 1))
	else
		echo "$LOC_OUTPUT"
		fail "LOC guard: some .rs files exceed 800 lines"
	fi
else
	warn "check-rs-loc.sh not found — skipping"
fi

# ── 9. Rust Version Sync ───────────────────────────────────────────────────
header "Rust Version Sync"
if [ -f scripts/gates/check-rust-version-sync.sh ]; then
	if bash scripts/gates/check-rust-version-sync.sh 2>&1; then
		info "Rust version: all sources in sync"
		PASS=$((PASS + 1))
	else
		fail "Rust version: sources out of sync"
	fi
else
	warn "check-rust-version-sync.sh not found — skipping"
fi

# ── 10. Documentation Disclaimer ───────────────────────────────────────────
header "Documentation Disclaimer"
if [ -f scripts/gates/inject-disclaimer.sh ]; then
	if bash scripts/gates/inject-disclaimer.sh --check 2>&1; then
		info "Documentation disclaimer: all .md files have the disclaimer"
		PASS=$((PASS + 1))
	else
		if $FIX_MODE; then
			# Idempotent injector: appends the standard disclaimer block to
			# headerless .md files. Excluded patterns stay excluded.
			if bash scripts/gates/inject-disclaimer.sh 2>&1 | tail -3; then
				warn "disclaimer: injector ran - re-run gatekeeper to verify"
			else
				fail "Documentation disclaimer: injector failed"
			fi
		else
			fail "Documentation disclaimer: some .md files missing the disclaimer"
			echo "  Fix: gate-keepers.sh --fix-all (or: bash scripts/gates/inject-disclaimer.sh)"
		fi
	fi
else
	warn "inject-disclaimer.sh not found — skipping"
fi

# ── 11. Symbol-Only Output Guard (v80.0.0-beta.2) ──────────────────────────
header "Symbol-Only Output"
if [ -f scripts/gates/check-symbol-only-output.sh ]; then
	if bash scripts/gates/check-symbol-only-output.sh 2>&1; then
		info "symbol-only: no icon glyphs in output surfaces"
		PASS=$((PASS + 1))
	else
		fail "symbol-only: icon glyphs found in output surfaces (v80.0.0-beta.2 rule)"
	fi
else
	warn "check-symbol-only-output.sh not found — skipping"
fi

# ── 12. Comment Style (markdown emphasis ban, 2026-09-04) ──────────────────
# Owner mandate: comments are plain prose — no **bold** / *italic* markers
# in any comment type. Functional rustdoc (backticks, fences, links,
# headings) is unaffected. See docs/COMMENT_STYLE.md section 2.
header "Comment Style (no markdown emphasis)"
if [ -f scripts/gates/check-comment-style.py ] && command -v python3 >/dev/null 2>&1; then
	if python3 scripts/gates/check-comment-style.py 2>&1; then
		PASS=$((PASS + 1))
	else
		fail "comment-style: decorative markdown emphasis in comments (see docs/COMMENT_STYLE.md)"
	fi
else
	warn "check-comment-style.py or python3 not found — skipping"
fi

# ── 13. Language Audit (pure-English rule, 2026-09-11) ─────────────────────
# Owner mandate (NIGHT-depthtest-2): commit messages, comments, strings,
# docs, and diagnostics are English only. The detector classifies
# non-ASCII content: letter runs of 2+ in a non-Latin script and
# non-allowlisted Latin diacritic words are language (fail); isolated
# math/unit letters, charset glyph data lines, and unicode-stress
# fixture files are functional (kept, each exemption carries its
# reason in the script).
header "Language Audit (pure English)"
if [ -f scripts/audit/language_audit.py ] && command -v python3 >/dev/null 2>&1; then
	if python3 scripts/audit/language_audit.py 2>&1; then
		PASS=$((PASS + 1))
	else
		fail "language-audit: non-English human language content found (see scripts/audit/language_audit.py)"
	fi
else
	warn "language_audit.py or python3 not found — skipping"
fi

# ── 14. File Permission Guard (644/755 owner rule, 2026-09-13) ─────────────
# Tracked files 644 (755 when executable), directories 755, shebang
# parity. Generalizes the EXE001 lesson from CI run #1484 beyond
# scripts/*.py to every tracked script, including .sh and benchmark/
# tools. --fix-all runs the guard with --fix (chmod in place; the
# 644/755 exec-bit flips are visible in git diff, the umask-level
# 664/775 repairs are invisible because git records only the exec bit).
header "Permission Guard (644/755)"
if [ -f scripts/gates/check-permissions.sh ]; then
	if $FIX_MODE; then
		if bash scripts/gates/check-permissions.sh --fix 2>&1; then
			info "permissions: violations auto-fixed (review git diff for exec-bit changes)"
			PASS=$((PASS + 1))
		else
			fail "permissions: violations not fully auto-fixable (review output above)"
		fi
	else
		if bash scripts/gates/check-permissions.sh 2>&1; then
			info "permissions: files 644, executables and directories 755"
			PASS=$((PASS + 1))
		else
			fail "permissions: violations found (auto-fixable via --fix-all)"
		fi
	fi
else
	warn "check-permissions.sh not found — skipping"
fi

# ── 15. Emoji Sweep (repo-wide, NIGHT-hunt-48) ─────────────────────────
# Owner rule (2026-09-14): the project carries no emoji anywhere —
# docs, source, scripts, configs alike; diagnostics already use the
# ASCII symbol vocabulary (check 11), this extends the ban to every
# tracked text file. The detector scans anything that decodes as
# strict UTF-8, so file types cannot escape it by extension.
# Excluded: docs/archive/** (frozen history), bench-labs artifacts,
# and two justified data exemptions (the denylist script itself +
# the message.rs sanitizer test input). --fix maps icons to OK/X.
header "Emoji Sweep (repo-wide)"
if [ -f scripts/audit/emoji-audit.py ] && command -v python3 >/dev/null 2>&1; then
	if python3 scripts/audit/emoji-audit.py 2>&1; then
		info "emoji sweep: no emoji-class codepoints in tracked text files"
		PASS=$((PASS + 1))
	else
		fail "emoji sweep: emoji found (python3 scripts/audit/emoji-audit.py --fix, then review the replacements)"
	fi
else
	warn "emoji-audit.py or python3 not found — skipping"
fi

# ── 16. CI Path-Filter Hygiene (glob-only, NIGHT-boost-1) ─────────────
# Owner mandate (2026-09-24): CI path filters gate on directories, not
# files. A `paths:` entry that names a specific file inside a directory
# (`scripts/example.sh`) silently rots when the file is renamed — the
# workflow stops triggering while the filter still looks alive. The
# detector flags any slash-carrying entry without a glob metacharacter
# in the paths/paths-ignore blocks of .github/workflows/*.yml;
# root-level entries (Cargo.toml, deny.toml) are exempt.
header "CI Path-Filter Hygiene (glob-only)"
if [ -f scripts/gates/check-ci-path-filters.py ] && command -v python3 >/dev/null 2>&1; then
	if python3 scripts/gates/check-ci-path-filters.py 2>&1; then
		PASS=$((PASS + 1))
	else
		fail "ci-path-filters: hardcoded filename entries in workflow path filters (see scripts/gates/check-ci-path-filters.py)"
	fi
else
	warn "check-ci-path-filters.py or python3 not found — skipping"
fi

# ── 17. Scripts LOC Guard (1000-line hard cap, NIGHT-lts-1) ─────────────
# Owner rule (2026-09-24): every shell and Python script under
# scripts/ (recursive - subdirectories included) stays at or below
# 1000 gross lines. Mirrors the Rust 800 cap of check 8: wc -l
# gross counting, # LOC_EXEMPT: marker exemptions as tracked debt,
# exit 1 on non-exempt violations. Capture-in-if pattern so a
# violation surfaces the full per-file output before the exit.
header "Scripts LOC Guard"
if [ -f scripts/gates/check-scripts-loc.sh ]; then
	if SLOC_OUTPUT=$(bash scripts/gates/check-scripts-loc.sh 2>&1); then
		info "scripts LOC guard: all script files ≤1000 lines"
		PASS=$((PASS + 1))
	else
		echo "$SLOC_OUTPUT"
		fail "scripts LOC guard: some scripts exceed 1000 lines without a # LOC_EXEMPT: marker"
	fi
else
	warn "check-scripts-loc.sh not found — skipping"
fi

# ── 18. Name-Case Rule (owner mandate, NIGHT-dinner-2) ─────────────────
# The project name is lowercase cosmostrix in every context — the
# nginx/curl convention docs/BRANDING.md section 2 codifies.
# check-name-case.py scans EVERY tracked file and every tracked
# path (git ls-files — the .cargo/ and .github/ hidden trees
# included, the archive included: nothing is excluded), classifies
# each name token case by case, and fails on any casing outside
# the legal families: lowercase, the identifier family
# (COSMOSTRIX_* environment variables, the COSMOSTRIX-* markers),
# the scripts/ banner titles, and the all-caps display banners the
# binary prints. Supersedes the old section 6c one-casing check.
header "Name-Case Rule (check-name-case.py)"
if [ -f scripts/gates/check-name-case.py ] && command -v python3 >/dev/null 2>&1; then
	if python3 scripts/gates/check-name-case.py 2>&1; then
		info "name-case: the project name is lowercase everywhere (identifier/banner/display families legal)"
		PASS=$((PASS + 1))
	else
		fail "name-case: wrong-case project name found (see the file:line:token list above)"
	fi
elif [ ! -f scripts/gates/check-name-case.py ]; then
	warn "check-name-case.py not found — skipping"
else
	warn "python3 not installed — skipping"
fi

# ── 19. Release-notes shape contract (dinner-11) ───────────────────────
# The release-body generator had zero automated coverage: a regression
# in its positional CLI or its render guards would only surface at the
# next release publish — exactly the geometry that killed zelynic's
# v11.0.0 stable (its generator's flag parser rejected the legal-empty
# --since-stable "" that only a stable cut produces; the rc series
# never tripped it). check-release-notes-shapes.sh re-executes the
# generator through its real CLI against a hermetic temp git repo for
# every documented shape, so the contract is enforced on every push
# instead of at the next tag.
header "Release-Notes Shape Contract (check-release-notes-shapes.sh)"
if [ -f scripts/gates/check-release-notes-shapes.sh ]; then
	if bash scripts/gates/check-release-notes-shapes.sh 2>&1; then
		info "release-notes shapes: the generator honors its full invocation contract"
		PASS=$((PASS + 1))
	else
		fail "release-notes shapes: the generator broke its shape contract (see the shape list above)"
	fi
else
	warn "check-release-notes-shapes.sh not found — skipping"
fi

# ── Summary ────────────────────────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════════════════════"
echo -e "  Gatekeeper Results: ${GREEN}${PASS} passed${NC}, ${RED}${FAIL} failed${NC}"
echo "═══════════════════════════════════════════════════════════════"

if [ "$FAIL" -gt 0 ]; then
	echo -e "${RED}COMMIT BLOCKED: ${FAIL} check(s) failed.${NC}"
	if ! $FIX_MODE; then
		echo "Fix the issues above, or run: ./scripts/gates/gate-keepers.sh --fix-all"
		echo "(codespell findings are never auto-fixed - review those manually)"
	else
		echo "Auto-fixes applied where possible; remaining findings need manual"
		echo "attention. Re-run plain ./scripts/gates/gate-keepers.sh to confirm."
	fi
	exit 1
else
	echo -e "${GREEN}All checks passed — safe to commit.${NC}"
	exit 0
fi
