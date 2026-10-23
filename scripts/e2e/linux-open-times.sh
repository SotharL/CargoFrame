#!/usr/bin/env bash
# How long the windows take to open, from the app's own timings (the log line "timing: usable"):
# shortcut to a usable CargoFrame window cold and warm, and the Settings window cold. Budgets come
# from the performance plan; the gate allows 20% over, times CARGOFRAME_PERF_SCALE (default 1).
#
#   scripts/e2e/linux-open-times.sh [path-to-release-binary]
#
# Run it against a RELEASE build (cargo build --release -p cargoframe --features
# tauri/custom-protocol). A virtual display has no graphics card, so web view start-up is slower
# here than on a desktop: CI uses a scale above 1 and docs/performance.md keeps the real numbers.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [ -z "${CARGOFRAME_TIMES_INNER:-}" ]; then
  bin="${1:-$root/target/release/cargoframe}"
  export CARGOFRAME_TIMES_INNER=1 CARGOFRAME_TIMES_BIN="$(readlink -f "$bin")"
  exec dbus-run-session -- xvfb-run -a -s "-screen 0 1280x800x24" "$0"
fi

bin="$CARGOFRAME_TIMES_BIN"
stub="$root/target/debug/examples/tray_host_stub"
fake="$root/target/debug/examples/fake_app"
[ -x "$stub" ] && [ -x "$fake" ] || { echo "build the helpers first: cargo build -p cargoframe-capture --examples" >&2; exit 1; }
scale="${CARGOFRAME_PERF_SCALE:-1}"

work="$(mktemp -d)"
cleanup() {
  for p in "${app_pid:-}" "${stub_pid:-}" "${fake_pid:-}"; do [ -n "$p" ] && kill "$p" 2>/dev/null || true; done
  rm -rf "$work"
}
trap cleanup EXIT
fail() { echo "FAIL: $*" >&2; [ -f "$work/app.log" ] && tail -n 30 "$work/app.log" >&2; exit 1; }

export HOME="$work/home"; mkdir -p "$HOME"
export CARGOFRAME_HOME="$work/cfg" CARGOFRAME_LOG=info CARGOFRAME_LOG_STDERR=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 GDK_BACKEND=x11

cfg="$CARGOFRAME_HOME/config"
mkdir -p "$cfg/triggers" "$cfg/layouts"
cat > "$cfg/triggers/bug.json" <<'JSON'
{ "schema_version": 1, "name": "Bug report", "enabled": true, "shortcut": "Ctrl+Shift+F9",
  "capture": "active_window", "layout_id": "bug" }
JSON
cat > "$cfg/layouts/bug.json" <<'JSON'
{ "blocks": [
  { "id": "shot", "type": "screenshot", "rect": { "x": 0, "y": 0, "w": 8, "h": 9 } },
  { "id": "prompt", "type": "text", "rect": { "x": 8, "y": 0, "w": 4, "h": 3 }, "title": "Prompt", "role": "prompt" },
  { "id": "send", "type": "sharing_button", "rect": { "x": 8, "y": 9, "w": 4, "h": 2 }, "label": "Share" }
] }
JSON

"$stub" > "$work/stub.out" 2>&1 & stub_pid=$!
for _ in $(seq 50); do grep -q ready "$work/stub.out" 2>/dev/null && break; sleep 0.1; done
grep -q ready "$work/stub.out" || fail "the tray host stub did not start"

mkdir -p "$work/dev/Foo"
cp "$fake" "$work/dev/Foo/Foo"
"$work/dev/Foo/Foo" "Foo - main.rs" > "$work/fake.out" 2>&1 & fake_pid=$!
for _ in $(seq 50); do grep -q ready "$work/fake.out" 2>/dev/null && break; sleep 0.1; done
grep -q ready "$work/fake.out" || fail "the fake program did not start"

"$bin" --autostart > "$work/app.log" 2>&1 & app_pid=$!
for _ in $(seq 100); do grep -qi "shortcut registered" "$work/app.log" && break; sleep 0.1; done
grep -qi "shortcut registered" "$work/app.log" || fail "the shortcut was not registered"
sleep 1

fake_win="$(xdotool search --pid "$fake_pid" | head -n1)"
focus_fake() { xdotool windowraise "$fake_win" || true; xdotool windowfocus "$fake_win" || true; sleep 0.4; }

# The nth "timing: usable" line for a metric, in ms; waits up to 30 seconds.
wait_usable() {   # metric, nth
  local metric="$1" nth="$2" line
  for _ in $(seq 300); do
    line="$(grep -E "timing: usable.*metric=\"?$metric\"?" "$work/app.log" | sed -n "${nth}p" || true)"
    if [ -n "$line" ]; then echo "$line" | grep -oE 'ms=[0-9]+' | head -n1 | cut -d= -f2; return 0; fi
    sleep 0.1
  done
  return 1
}
ready_ms() { grep -oE 'ready_ms=[0-9]+' "$work/app.log" | sed -n "${1}p" | cut -d= -f2; }

gate() {   # name, measured ms, budget ms
  local limit
  limit="$(awk -v b="$3" -v s="$scale" 'BEGIN { printf "%d", b * 1.2 * s }')"
  echo "$1: ${2} ms (budget $3 ms, gate ${limit} ms)"
  [ "$2" -le "$limit" ] || fail "$1 took ${2} ms, over the gate of ${limit} ms"
  summary+="- $1: ${2} ms (budget $3 ms, gate ${limit} ms)"$'\n'
}
summary=""

# 1. Cold: the first shortcut press builds the window.
focus_fake
xdotool key --clearmodifiers ctrl+shift+F9
cold="$(wait_usable paint 1)" || fail "no usable CargoFrame window after the first shortcut press"
gate "Shortcut to capture ready" "$(ready_ms 1)" 150
gate "Shortcut to usable window (cold)" "$cold" 700

# 2. Warm: Escape hides the window (Balanced); the next press only shows it again.
for _ in $(seq 8); do
  xdotool key --clearmodifiers Escape
  sleep 0.6
  grep -q "window closing" "$work/app.log" 2>/dev/null && break
done
sleep 0.5
focus_fake
xdotool key --clearmodifiers ctrl+shift+F9
warm="$(wait_usable paint 2)" || fail "no usable window after the second shortcut press"
gate "Shortcut to usable window (warm)" "$warm" 250

# 3. Settings, cold: another launch asks the running one to open it.
"$bin" > /dev/null 2>&1 || true
settings="$(wait_usable settings_paint 1)" || fail "Settings did not open"
gate "Settings window open (cold)" "$settings" 500

if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  { echo "### Window open times (Linux, virtual display, scale x$scale)"; printf '%s' "$summary"; } >> "$GITHUB_STEP_SUMMARY"
fi
"$bin" --quit >/dev/null 2>&1 || true
for _ in $(seq 50); do kill -0 "$app_pid" 2>/dev/null || break; sleep 0.1; done
app_pid=""
echo "Linux open-times check passed"
