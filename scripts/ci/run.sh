#!/usr/bin/env bash
# Run a command; if it fails, repeat the end of its output as a GitHub annotation.
#
# Why: annotations are the part of a run that can be read without downloading the log archive
# (which some environments cannot reach). Usage: scripts/ci/run.sh "Title" command args...
set -u
title="$1"
shift
log="$(mktemp)"
"$@" 2>&1 | tee "$log"
code=${PIPESTATUS[0]}
if [ "$code" -ne 0 ]; then
  msg="$(tail -n 120 "$log" \
    | sed -e 's/\x1b\[[0-9;]*[A-Za-z]//g' -e 's/\r$//' -e 's/%/%25/g' \
    | awk '{ printf "%s%%0A", $0 }' | cut -c1-20000)"
  echo "::error title=${title} failed (exit ${code})::${msg}"
fi
rm -f "$log"
exit "$code"
