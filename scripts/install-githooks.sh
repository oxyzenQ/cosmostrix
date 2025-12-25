#!/bin/sh
set -eu

ROOT=$(git rev-parse --show-toplevel 2>/dev/null || true)
if [ -z "${ROOT}" ]; then
    echo "Not inside a git repository" >&2
    exit 1
fi

cd "$ROOT"

if [ ! -d ".githooks" ]; then
    echo "Missing .githooks directory" >&2
    exit 1
fi

if [ ! -f ".githooks/pre-push" ]; then
    echo "Missing .githooks/pre-push hook" >&2
    exit 1
fi

git config core.hooksPath .githooks

chmod +x .githooks/pre-push || true
chmod +x ./build.sh || true
chmod +x ./scripts/install-githooks.sh || true

echo "Git hooks installed (core.hooksPath=.githooks)"
echo "To skip pre-push checks for a single push, use: COSMOSTRIX_SKIP_PREPUSH=1 git push"
