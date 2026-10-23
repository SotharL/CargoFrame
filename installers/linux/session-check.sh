#!/usr/bin/env bash
# Runs inside a session bus and a virtual display (see smoke-test.sh): start the installed app the
# way sign-in would, check it stays up, ask it to quit, check it does.
set -euo pipefail
prog="$1"
work="$2"
log="$work/app.log"

"$prog" --autostart >"$log" 2>&1 &
pid=$!
sleep 8
if ! kill -0 "$pid" 2>/dev/null; then
  echo "FAIL: the app exited by itself"
  cat "$log"
  exit 1
fi
echo "ok: app stays running after --autostart"

"$prog" --quit >"$work/quit.log" 2>&1 || true
for _ in $(seq 1 20); do
  kill -0 "$pid" 2>/dev/null || break
  sleep 0.5
done
if kill -0 "$pid" 2>/dev/null; then
  echo "FAIL: the app did not quit on --quit"
  echo "--- app.log"; cat "$log"; echo "--- quit.log"; cat "$work/quit.log"
  kill "$pid" 2>/dev/null || true
  exit 1
fi
echo "ok: --quit stops the running copy"
