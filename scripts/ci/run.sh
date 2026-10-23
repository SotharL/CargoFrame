#!/usr/bin/env bash
# Run a command; if it fails, repeat what matters of its output as a GitHub annotation.
#
# Why: annotations are the part of a run that can be read without downloading the log archive
# (which some environments cannot reach). GitHub keeps only the first 4096 characters of an
# annotation, so the message is built to fit: the end of the output first (where the failure
# usually is), then the lines that name failures, last.
# Usage: scripts/ci/run.sh "Title" command args...
set -u
title="$1"
shift
log="$(mktemp)"
"$@" 2>&1 | tee "$log"
code=${PIPESTATUS[0]}
if [ "$code" -ne 0 ]; then
  clean() { sed -e 's/\x1b\[[0-9;]*[A-Za-z]//g' -e 's/\r$//' -e 's/%/%25/g'; }
  failing="$(clean <"$log" | grep -E 'FAILED|panicked at|^error(\[|:)|---- |assertion|left:|right:' | tail -n 25 | cut -c1-300)"
  # room for the failing lines is reserved first, so they are never the part that is cut
  budget=$((3800 - ${#failing}))
  [ "$budget" -lt 800 ] && budget=800
  tailpart="$(clean <"$log" | tail -n 60 | cut -c1-300 | tail -c "$budget")"
  msg="$(printf '%s\n=== failing lines ===\n%s\n' "$tailpart" "$failing" | awk '{ printf "%s%%0A", $0 }')"
  echo "::error title=${title} failed (exit ${code})::${msg}"
fi
rm -f "$log"
exit "$code"
