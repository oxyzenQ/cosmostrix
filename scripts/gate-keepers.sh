#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# Pre-commit gatekeeper script for cosmostrix.
# Runs all non-code linters/checks before allowing a commit.
#
# Usage:
#   ./scripts/gate-keepers.sh           # Run all checks
#   ./scripts/gate-keepers.sh --fix    # Run with auto-fix where possible
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
# commands the CI job runs, over the same file set (scripts/*.py).
header "Python lint (ruff)"
PY_FILES=$(find scripts -maxdepth 1 -name '*.py' 2>/dev/null)
if [ -n "$PY_FILES" ]; then
        if command -v ruff >/dev/null 2>&1; then
                RUFF_OK=0
                if $FIX_MODE; then
                        # Auto-fix: apply lint fixes (safe rules only) + format.
                        if ! ruff check --fix scripts/*.py 2>&1; then
                                fail "ruff check: unfixable python lint errors remain (fix manually)"
                                RUFF_OK=1
                        fi
                        ruff format scripts/*.py 2>&1 || true
                else
                        if ! ruff check scripts/*.py 2>&1; then
                                fail "ruff check: python lint errors found (auto-fixable via --fix-all)"
                                RUFF_OK=1
                        fi
                        if ! ruff format --check scripts/*.py 2>&1; then
                                fail "ruff format: python files not formatted (auto-fixable via --fix-all)"
                                RUFF_OK=1
                        fi
                fi
                # CI parity guard: ruff's EXE001 flags shebang'd files that lack the
                # executable bit. Same rule, checked locally so the gatekeeper
                # catches it before CI does (incident run #1484).
                for py in scripts/*.py; do
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

# ── 6c. Naming consistency ─────────────────────────────────────────────────
# The project name is always lowercase `cosmostrix` (owner rule 2026-08-24,
# documented in CONTRIBUTING.md section 2). The capitalized form is a naming
# inconsistency; archived historical documents are exempt. The pattern below
# is written without the literal capitalized form so this check does not
# match its own source.
header "Naming consistency"
# `git grep` exits 1 on zero matches (the clean state) - guard with || true
# so `set -e` does not kill the script on success.
CAP_HITS=$(git grep -l -E "C""osmostrix" -- ":(exclude)docs/archive/**" 2>/dev/null | head -5 || true)
if [ -z "$CAP_HITS" ]; then
        info "naming: project name is lowercase everywhere (non-archive)"
        PASS=$((PASS + 1))
else
        echo "$CAP_HITS" | while IFS= read -r f; do
                echo "  capitalized project name in: $f"
        done
        fail "naming: use lowercase 'cosmostrix' (see CONTRIBUTING.md section 2)"
fi

# ── 7. SPDX License Header Check ──────────────────────────────────────────
header "SPDX License Headers"
if [ -f scripts/check-headers.sh ]; then
        if bash scripts/check-headers.sh 2>&1; then
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
header "LOC Guard"
if [ -f scripts/check-rs-loc.sh ]; then
        if bash scripts/check-rs-loc.sh 2>&1 | tail -3; then
                info "LOC guard: all .rs files ≤800 lines"
                PASS=$((PASS + 1))
        else
                fail "LOC guard: some .rs files exceed 800 lines"
        fi
else
        warn "check-rs-loc.sh not found — skipping"
fi

# ── 9. Rust Version Sync ───────────────────────────────────────────────────
header "Rust Version Sync"
if [ -f scripts/check-rust-version-sync.sh ]; then
        if bash scripts/check-rust-version-sync.sh 2>&1; then
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
if [ -f scripts/inject-disclaimer.sh ]; then
        if bash scripts/inject-disclaimer.sh --check 2>&1; then
                info "Documentation disclaimer: all .md files have the disclaimer"
                PASS=$((PASS + 1))
        else
                if $FIX_MODE; then
                        # Idempotent injector: appends the standard disclaimer block to
                        # headerless .md files. Excluded patterns stay excluded.
                        if bash scripts/inject-disclaimer.sh 2>&1 | tail -3; then
                                warn "disclaimer: injector ran - re-run gatekeeper to verify"
                        else
                                fail "Documentation disclaimer: injector failed"
                        fi
                else
                        fail "Documentation disclaimer: some .md files missing the disclaimer"
                        echo "  Fix: gate-keepers.sh --fix-all (or: bash scripts/inject-disclaimer.sh)"
                fi
        fi
else
        warn "inject-disclaimer.sh not found — skipping"
fi

# ── 11. Symbol-Only Output Guard (v80.0.0-beta.2) ──────────────────────────
header "Symbol-Only Output"
if [ -f scripts/check-symbol-only-output.sh ]; then
        if bash scripts/check-symbol-only-output.sh 2>&1; then
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
if [ -f scripts/check-comment-style.py ] && command -v python3 >/dev/null 2>&1; then
        if python3 scripts/check-comment-style.py 2>&1; then
                PASS=$((PASS + 1))
        else
                fail "comment-style: decorative markdown emphasis in comments (see docs/COMMENT_STYLE.md)"
        fi
else
        warn "check-comment-style.py or python3 not found — skipping"
fi

# ── Summary ────────────────────────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════════════════════"
echo -e "  Gatekeeper Results: ${GREEN}${PASS} passed${NC}, ${RED}${FAIL} failed${NC}"
echo "═══════════════════════════════════════════════════════════════"

if [ "$FAIL" -gt 0 ]; then
        echo -e "${RED}COMMIT BLOCKED: ${FAIL} check(s) failed.${NC}"
        if ! $FIX_MODE; then
                echo "Fix the issues above, or run: ./scripts/gate-keepers.sh --fix-all"
                echo "(codespell findings are never auto-fixed - review those manually)"
        else
                echo "Auto-fixes applied where possible; remaining findings need manual"
                echo "attention. Re-run plain ./scripts/gate-keepers.sh to confirm."
        fi
        exit 1
else
        echo -e "${GREEN}All checks passed — safe to commit.${NC}"
        exit 0
fi
