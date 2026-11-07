#!/usr/bin/env bash
# The local gate: everything CI does on Linux, run on this computer first.
#
#   scripts/dev/check.sh           everything, including the capture end-to-end check
#   scripts/dev/check.sh quick     format, type check and UI tests only (seconds)
#   scripts/dev/check.sh rust      format, lint and every Rust test
#
# Push only when this passes: CI minutes are limited, so GitHub should confirm a result, not find it.
set -u
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
mode="${1:-all}"
case "$mode" in all | quick | rust) ;; *) echo "usage: check.sh [all|quick|rust]" >&2; exit 2 ;; esac

fails=()
log="$(mktemp)"
trap 'rm -f "$log"' EXIT
step() {
  local name="$1"; shift
  local t0=$SECONDS
  if "$@" >"$log" 2>&1; then
    printf 'PASS  %-34s %4ss\n' "$name" "$((SECONDS - t0))"
  else
    printf 'FAIL  %-34s %4ss\n' "$name" "$((SECONDS - t0))"
    tail -n 30 "$log" | sed 's/^/      | /'
    fails+=("$name")
  fi
}
rust_tests() {
  cargo test --workspace --no-fail-fast 2>&1 | tee "$log.t" | grep -E '^test result' | awk '{p+=$4; f+=$6} END {print p" passed, "f" failed"; exit f>0}'
  local code=${PIPESTATUS[0]}
  grep -E 'FAILED|panicked' "$log.t" | head -10
  rm -f "$log.t"
  return "$code"
}

step "cargo fmt --check" cargo fmt --all --check
if [ "$mode" != quick ]; then
  step "cargo clippy -D warnings" cargo clippy --workspace --all-targets -- -D warnings
  step "cargo test (all crates)" rust_tests
fi
if [ "$mode" != rust ]; then
  step "file names (Windows-safe)" node scripts/ci/check-names.mjs
  step "svelte-check" bash -c 'cd ui && npx svelte-check'
  step "vitest" bash -c 'cd ui && npx vitest run'
fi
if [ "$mode" = all ]; then
  step "ui build" bash -c 'cd ui && npm run build'
  step "size budgets" node scripts/ci/check-budgets.mjs
  step "build app (embedded UI)" cargo build -p cargoframe --features tauri/custom-protocol
  step "build test helper" cargo build -p cargoframe-capture --example fake_app
  if command -v xvfb-run >/dev/null && command -v xdotool >/dev/null; then
    step "e2e capture (virtual display)" timeout 240 bash scripts/e2e/linux-capture.sh
  else
    echo "SKIP  e2e capture (needs xvfb and xdotool)"
  fi
fi

echo
if [ "${#fails[@]}" -eq 0 ]; then
  echo "All checks passed ($mode). Safe to push."
else
  echo "Failed: ${fails[*]}"
  exit 1
fi
