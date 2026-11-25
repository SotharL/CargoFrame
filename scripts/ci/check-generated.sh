#!/usr/bin/env bash
# The window's copies of Rust types and defaults (ui/src/lib/generated) are written by
# `cargo test` (ts-rs). Run after the tests: in CI the files must be the committed ones, so a
# Rust type changed without its copy fails here instead of in the window.
#   scripts/ci/check-generated.sh          # fail when they differ from the commit (CI)
#   scripts/ci/check-generated.sh --note   # only say so (check.sh, before a commit)
set -euo pipefail
cd "$(dirname "$0")/../.."
changed="$(git status --porcelain -- ui/src/lib/generated)"
if [ -z "$changed" ]; then
  echo "ui/src/lib/generated matches the Rust types"
  exit 0
fi
echo "ui/src/lib/generated differs from the commit (cargo test wrote it from the Rust types):"
echo "$changed"
if [ "${1:-}" = "--note" ]; then
  echo "commit these files with the change"
  exit 0
fi
git --no-pager diff -- ui/src/lib/generated | head -60
exit 1
