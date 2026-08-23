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
#   1.  shellcheck on all .sh files
#   2.  yamllint on all .yml/.yaml files
#   3.  actionlint on all .github/workflows/*.yml
#   4.  TOML syntax validation (python3 tomllib)
#   5.  markdownlint on all .md files
#   6.  codespell on all text files
#   7.  SPDX license header check
#   8.  LOC guard (1500-line cap on .rs files)
#   9.  Rust version sync check
#  10.  Documentation disclaimer check (all .md files have the
#       "source code is truth, cross-check before relying" disclaimer)
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

if [[ "${1:-}" == "--fix" ]]; then
    FIX_MODE=true
fi

info()  { echo -e "${GREEN}[PASS]${NC} $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
fail()  { echo -e "${RED}[FAIL]${NC} $1"; FAIL=$((FAIL + 1)); }
header() { echo ""; echo "── $1 ──"; }

# ── 1. Shellcheck ──────────────────────────────────────────────────────────
header "Shellcheck"
if command -v shellcheck >/dev/null 2>&1; then
    SHELL_FILES=$(find . -name '*.sh' -not -path './.git/*' -not -path './target/*' 2>/dev/null)
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

# ── 2. Yamllint ────────────────────────────────────────────────────────────
header "Yamllint"
if command -v yamllint >/dev/null 2>&1; then
    YAML_FILES=$(find .github aur .cargo -name '*.yml' -o -name '*.yaml' 2>/dev/null)
    if [ -n "$YAML_FILES" ]; then
        # shellcheck disable=SC2086 # word splitting is intentional for file list
        if yamllint -d "{extends: default, rules: {line-length: disable, document-start: disable, truthy: disable}}" ${YAML_FILES} 2>&1; then
            info "yamllint: all YAML files pass"
            PASS=$((PASS + 1))
        else
            fail "yamllint: errors found in YAML files"
        fi
    else
        info "yamllint: no YAML files found"
        PASS=$((PASS + 1))
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
        fail "codespell: spelling errors found"
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
        if ! ruff check scripts/*.py 2>&1; then
            fail "ruff check: python lint errors found (run: ruff check --fix scripts/*.py)"
            RUFF_OK=1
        fi
        if ! ruff format --check scripts/*.py 2>&1; then
            fail "ruff format: python files not formatted (run: ruff format scripts/*.py)"
            RUFF_OK=1
        fi
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
if [ -f scripts/check-headers.sh ]; then
    if bash scripts/check-headers.sh 2>&1; then
        info "SPDX headers: all files have license headers"
        PASS=$((PASS + 1))
    else
        fail "SPDX headers: some files missing license headers"
    fi
else
    warn "check-headers.sh not found — skipping"
fi

# ── 8. LOC Guard (1500-line cap) ──────────────────────────────────────────
header "LOC Guard"
if [ -f scripts/check-rs-loc.sh ]; then
    if bash scripts/check-rs-loc.sh 2>&1 | tail -3; then
        info "LOC guard: all .rs files ≤1500 lines"
        PASS=$((PASS + 1))
    else
        fail "LOC guard: some .rs files exceed 1500 lines"
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
        fail "Documentation disclaimer: some .md files missing the disclaimer"
        echo "  Fix: bash scripts/inject-disclaimer.sh"
    fi
else
    warn "inject-disclaimer.sh not found — skipping"
fi

# ── Summary ────────────────────────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════════════════════"
echo -e "  Gatekeeper Results: ${GREEN}${PASS} passed${NC}, ${RED}${FAIL} failed${NC}"
echo "═══════════════════════════════════════════════════════════════"

if [ "$FAIL" -gt 0 ]; then
    echo -e "${RED}COMMIT BLOCKED: ${FAIL} check(s) failed.${NC}"
    echo "Fix the issues above, then re-run ./scripts/gate-keepers.sh"
    exit 1
else
    echo -e "${GREEN}All checks passed — safe to commit.${NC}"
    exit 0
fi
